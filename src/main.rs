mod slack;
mod canvas;
mod stage;

use std::env;
use dotenv::dotenv;
use chrono::prelude::*;

use std::collections::HashMap;
use std::sync::{ Arc, Mutex };

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // .envから各種アクセストークンの取得
    dotenv().ok();
    let slack_app_token = env::var("SLACK_APP_TOKEN").expect("SLACK_APP_TOKEN must be set");
    let slack_bot_token = env::var("SLACK_BOT_TOKEN").expect("SLACK_BOT_TOKEN must be set");

    // オブジェクトの形状を読み込み
    let shapes = canvas::Canvas::load_shaper_from_svg("resources/shapes.svg", 3.0)?;

    // 各チャンネルごとに独立したステージを管理
    struct ChannelStage {
        update_time: DateTime<Local>,
        channel_id: String,
        stage: Option<stage::Stage>,
    }
    let stages = Arc::new(Mutex::new(HashMap::<String, Arc<tokio::sync::Mutex<ChannelStage>>>::new()));

    // メンションが送られてきたときに呼ばれる関数
    async fn compute_turn(
        bot_token: String,
        shapes: Vec<Vec<(f64, f64)>>,
        channel_stage: Arc<tokio::sync::Mutex<ChannelStage>>,
        message: slack::Message
    ) -> slack::SlackResult {
        if message.event_type != "app_mention" { return Ok(()); }
        let re = regex::Regex::new(r"^<@[0-9A-Z]+>").unwrap();
        let text = re.replace(&message.text, "").to_string();

        // 物理演算の結果を返す前に他の人のターンが重なるのを防ぐ
        if let Ok(mut channel_stage) = channel_stage.try_lock() {
            if let Some(stage) = &mut channel_stage.stage {
                // アイコン画像の登録
                if !stage.user_icons.contains_key(&message.user_id) {
                    let user_info = slack::get_user_info(bot_token.clone(), message.user_id.clone()).await?;
                    if let Some(icon_data) = user_info.icon_data {
                        stage.user_icons.insert(message.user_id.clone(), icon_data);
                    }
                }

                // メッセージの解析
                let args: Vec<&str> = text.split_whitespace().collect();
                if args.len() != 2 {
                    slack::post_message(bot_token.clone(), message.channel_id,
                        "無効な入力です。".to_string()
                    ).await?;
                    return Ok(());
                }
                let args = args[0..2].iter().map(|arg| arg.trim().parse::<f64>()).collect::<Result<Vec<f64>, std::num::ParseFloatError>>();
                let translation_x;
                let rotation;
                if let Ok(args) = args { translation_x = args[0]; rotation = args[1] } else {
                    slack::post_message(bot_token.clone(), message.channel_id,
                        "無効な入力です。".to_string()
                    ).await?;
                    return Ok(());
                }

                // 物理演算
                if let Ok((turn_result, height, data)) =
                    stage.next_turn(Some(message.user_id.clone()), translation_x as stage::Real, rotation as stage::Real)
                {
                    let result_message = match turn_result {
                        stage::TurnResult::Success => { format!("{} m", height) },
                        stage::TurnResult::Failure => { "Game Over :angry:".to_string() },
                        stage::TurnResult::Timeout => { "物理演算がタイムアウトしました:confounded:".to_string() },
                    };
                    let result_message = format!("<@{}> {}", message.user_id.clone(), result_message);
                    slack::post_image(bot_token.clone(), channel_stage.channel_id.clone(), result_message, &data, "result.png".to_string()).await?;

                    // ゲームオーバーまたはタイムアウトの場合はステージをリセット
                    if turn_result != stage::TurnResult::Success {
                        channel_stage.stage = None;
                    }
                }
            }
            else {
                // ステージが存在しなかった場合は生成
                let mut stage = stage::Stage::new(shapes);
                let (_, _, data) = stage.next_turn(None, 0.0, 0.0)?;
                channel_stage.stage = Some(stage);
                slack::post_image(bot_token.clone(), message.channel_id,
                    ":sparkles: slack tower battleへようこそ :sparkles:\n".to_string() +
                    "みんなでオブジェクトを積み重ねて高みを目指しましょう:fire: :fire: :fire:\n\n" +
                    "【遊び方】\n" +
                    "左右の位置(-1〜1) と回転角度(-180〜180、時計回りが正の回転) を送信してください。\n" +
                    "コマンド例 :point_right: `@slack_tower_battle -0.25 45`",
                &data, "result.png".to_string()).await?;
            }

            channel_stage.update_time = Local::now();
        }
        else {
            slack::post_message(bot_token.clone(), message.channel_id,
                format!("<@{}> 現在計算中です。\n結果が投稿された後に再度お試しください。", message.user_id)
            ).await?;
        }
        Ok(())
    }

    // 24時間以上経過したステージを自動削除するタスク
    async fn stage_cleaner(stages: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<ChannelStage>>>>>) {
        loop {
            let current_time = Local::now();
            let mut delete_channels = Vec::<String>::new();
            {
                if let Ok(stages) = &mut stages.lock() {
                    for (channel_id, channel_stage) in stages.iter() {
                        if let Ok(channel_stage) = channel_stage.try_lock() {
                            let elapsed_time = current_time - channel_stage.update_time;
                            if elapsed_time.num_hours() >= 24 { delete_channels.push(channel_id.clone()); }
                        }
                    }
                    for channel_id in delete_channels.iter() {
                        stages.remove(channel_id);
                        println!("delete: channel {}", channel_id);
                    }
                }
            }
            // 60秒おきに監視
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }
    tokio::spawn(stage_cleaner(Arc::clone(&stages)));

    // slackから取得したwebsocketのURLに接続
    slack::websocket_receiver(slack_app_token.clone(), |message| {
        let stages = Arc::clone(&stages);
        let stages = stages.lock();
        if let Ok(mut stages) = stages {
            if !stages.contains_key(&message.channel_id) {
                stages.insert(message.channel_id.clone(), Arc::new(tokio::sync::Mutex::new(ChannelStage{
                    update_time: Local::now(),
                    channel_id: message.channel_id.clone(),
                    stage: None,
                })));
            }

            if let Some(channel_stage) = stages.get(&message.channel_id) {
                tokio::spawn(compute_turn(slack_bot_token.clone(), shapes.clone(), Arc::clone(channel_stage), message));
            }
        }
    }).await;

    Ok(())
}

extern crate base64;
use std::error::Error;
use std::fmt;
use std::collections::HashMap;

#[derive(Debug)]
pub struct SlackError(String);
impl fmt::Display for SlackError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "There is an error: {}", self.0)
    }
}
impl Error for SlackError {}

pub type SlackResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub async fn get_websocket_url(app_token: String) -> SlackResult<String> {
    // slackからwebsocketのURLを取得
    // 参考: https://api.slack.com/apis/connections/socket-implement
    let client = reqwest::Client::new();
    let response_json = client.post("https://slack.com/api/apps.connections.open")
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", app_token))
        .body("").send().await?.text().await?;
    let response: serde_json::Value = serde_json::from_str(&response_json)?;
    let websocket_url = match response.get("url").and_then(|value| { value.as_str() }) {
        Some(url) => Ok(url.replace("\"", "")),
        None => Err(Box::new(SlackError(response_json))),
    }?;
    Ok(websocket_url)
}

pub async fn post_message(bot_token: String, channel: String, text: String) -> SlackResult {
    // slackにメッセージを送信
    // 参考: https://api.slack.com/methods/chat.postMessage
    let mut params = HashMap::new();
    params.insert("channel", channel);
    params.insert("text", text);
    let client = reqwest::Client::new();
    let response_json = client.post("https://slack.com/api/chat.postMessage")
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", bot_token))
        .form(&params).send().await?.text().await?;
    let response: serde_json::Value = serde_json::from_str(&response_json)?;
    if let Some(ok) = response.get("ok") {
        if let Some(ok) = ok.as_bool() {
            if ok { return Ok(()) }
        }
    }
    Err(Box::new(SlackError(response_json)))
}

pub async fn post_image(bot_token: String, channel: String, text: String, filedata: &Vec<u8>, filename: String) -> SlackResult {
    // slackに画像を送信
    // 参考: https://api.slack.com/methods/files.upload
    let form = reqwest::multipart::Form::new();
    let form = form.text("channels", channel.to_string());
    let form = form.text("initial_comment", text.to_string());
    let form = form.part("file", reqwest::multipart::Part::bytes(filedata.to_vec()).file_name(filename.to_string()));
    let client = reqwest::Client::new();
    let response_json = client.post("https://slack.com/api/files.upload")
        .header(reqwest::header::CONTENT_TYPE, "multipart/form-data")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", bot_token))
        .multipart(form).send().await?.text().await?;
    let response: serde_json::Value = serde_json::from_str(&response_json)?;
    if let Some(ok_val) = response.get("ok") {
        if let Some(ok) = ok_val.as_bool() {
            if ok { return Ok(()) }
        }
    }
    Err(Box::new(SlackError(response_json)))
}

#[derive(Debug)]
pub struct UserInfo {
    pub user_id: String,
    pub name: Option<String>,
    pub icon_data: Option<Vec<u8>>,
}
pub async fn get_user_info(bot_token: String, user_id: String) -> SlackResult<UserInfo> {
    // slackのuser_idからユーザー名とアイコン画像を取得
    // 参考: https://api.slack.com/methods/users.profile.get
    let client = reqwest::Client::new();
    let response_json = client.get("https://slack.com/api/users.profile.get")
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", bot_token))
        .query(&[("user", &user_id)]).send().await?.text().await?;
    let response: serde_json::Value = serde_json::from_str(&response_json)?;

    /*
    APIで返されるjsonの例

	{
		"ok": true,
		"profile": {
			"avatar_hash": "ge3b51ca72de",
			"status_text": "Print is dead",
			"status_emoji": ":books:",
			"status_expiration": 0,
			"real_name": "Egon Spengler",
			"display_name": "spengler",
			"real_name_normalized": "Egon Spengler",
			"display_name_normalized": "spengler",
			"email": "spengler@ghostbusters.example.com",
			"image_original": "https://.../avatar/e3b51ca72dee4ef87916ae2b9240df50.jpg",
			"image_24": "https://.../avatar/e3b51ca72dee4ef87916ae2b9240df50.jpg",
			"image_32": "https://.../avatar/e3b51ca72dee4ef87916ae2b9240df50.jpg",
			"image_48": "https://.../avatar/e3b51ca72dee4ef87916ae2b9240df50.jpg",
			"image_72": "https://.../avatar/e3b51ca72dee4ef87916ae2b9240df50.jpg",
			"image_192": "https://.../avatar/e3b51ca72dee4ef87916ae2b9240df50.jpg",
			"image_512": "https://.../avatar/e3b51ca72dee4ef87916ae2b9240df50.jpg",
			"team": "T012AB3C4"
		}
	}
    */

    let mut user_info = UserInfo{ user_id: user_id.to_string(), name: None, icon_data: None };
    if let Some(profile) = response.get("profile") {
        if let Some(name) = profile.get("display_name") {
            if let Some(name) = name.as_str() { user_info.name = Some(name.to_string()); }
            if name == "" {
                if let Some(name) = profile.get("real_name") {
                    if let Some(name) = name.as_str() { user_info.name = Some(name.to_string()); }
                }
            }
        }
        if let Some(image_url) = None.or(
            profile.get("image_original")).or(
            profile.get("image_1024")).or(
            profile.get("image_512")).or(
            profile.get("image_192")).or(
            profile.get("image_72")).or(
            profile.get("image_48")).or(
            profile.get("image_32")).or(
            profile.get("image_24"))
        {
            if let Some(image_url) = image_url.as_str() {
                let data = download_data(&image_url.to_string()).await?;
                user_info.icon_data = Some(data);
            }
        }
    }
    Ok(user_info)
}

pub async fn download_data(url: &String) -> SlackResult<Vec<u8>> {
    let response = reqwest::get(url).await?;
    Ok(response.bytes().await?.to_vec())
}

#[derive(Debug, Clone, Copy)]
enum Disconnect{ Reconnecting, Exit }
#[derive(Debug)]
pub struct Message {
    pub event_type: String,
    pub channel_id: String,
    pub user_id: String,
    pub text: String,
}
use futures_util::{pin_mut, StreamExt};
use tokio_tungstenite::tungstenite::protocol;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
async fn single_websocket_receiver(id: u64, app_token: String, sender: Sender<Message>) -> SlackResult<Disconnect> {
    // websocketのURLを取得
    println!("status(id: {}): connecting websocket", id);
    let url = get_websocket_url(app_token).await;
    if let Err(err) = url { return Err(err); }
    let websocket_url = url.unwrap();

    // websocketに接続
    //let url = url::Url::parse(&format!("{}&debug_reconnects=true", websocket_url)).unwrap();
    let url = url::Url::parse(&format!("{}", websocket_url)).unwrap();
    let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
    println!("status(id: {}): connected websocket", id);

    // 再接続フラグ
    let reconnect = Arc::new(AtomicBool::new(false));

    let (write, read) = ws_stream.split();
    let (responder_tx, responder_rx) = futures_channel::mpsc::channel(128);
    let responder = responder_rx.map(Ok).forward(write);
    let receiver = read.for_each(|message| async {
        if let Ok(protocol::Message::Text(json)) = message {
            let json: serde_json::Value = serde_json::from_str(&json).unwrap();
            // メッセージを受け取ったことをslackにレスポンスする
            // 参考: https://api.slack.com/apis/connections/socket-implement#acknowledge
            if let Some(envelope_id) = json.get("envelope_id") {
                let _ = responder_tx.clone().try_send(protocol::Message::Text(serde_json::json!({"envelope_id": envelope_id}).to_string()));
            }

            let message_type =
                if let Some(value) = json.get("type") { value.as_str() }
                else { None };
            match message_type {
                Some("events_api") => {
                    let event_type =
                        if let Some(value) = json.pointer("/payload/event/type") { value.as_str() }
                        else { None };
                    let channel_id =
                        if let Some(value) = json.pointer("/payload/event/channel") { value.as_str() }
                        else { None };
                    let user_id =
                        if let Some(value) = json.pointer("/payload/event/user") { value.as_str() }
                        else { None };
                    let text =
                        if let Some(value) = json.pointer("/payload/event/text") { value.as_str() }
                        else { None };
                    match (event_type, channel_id, user_id, text) {
                        (Some(event_type), Some(channel_id), Some(user_id), Some(text)) => {
                            let message = Message {
                                event_type: event_type.to_string(),
                                channel_id: channel_id.to_string(),
                                user_id: user_id.to_string(),
                                text: text.to_string(),
                            };
                            println!("received(id: {}): message {:?}", id, message);
                            let _ = sender.clone().try_send(message);
                        },
                        _ => {},
                    };
                },
                Some("disconnect") => {
                    let message_reason =
                        if let Some(value) = json.get("reason") { value.as_str() }
                        else { None };
                    match message_reason {
                        Some("warning") | Some("refresh_requested") => {
                            println!("received(id: {}): refresh request", id);
                            Arc::clone(&reconnect).store(true, std::sync::atomic::Ordering::SeqCst);
                            //let _ = responder_tx.clone().try_send(protocol::Message::Close(None));
                        },
                        _ => {},
                    };
                },
                _ => {},
            };
        }
    });
    pin_mut!(receiver, responder);
    future::select(receiver, responder).await;
    println!("status(id: {}): disconnected", id);
    if reconnect.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(Disconnect::Reconnecting);
    }
    Ok(Disconnect::Exit)
}

use futures::future;
use futures_channel::mpsc::{ channel, Sender };
pub async fn websocket_receiver<F: Fn(Message)>(app_token: String, message_handler: F) {
    async fn auto_reconnecting(id: u64, app_token: String, sender: Sender<Message>) {
        //while let Ok(Disconnect::Reconnecting) = single_websocket_receiver(id, app_token.clone(), sender.clone()).await {}
        loop{
            if let Ok(Disconnect::Reconnecting) = single_websocket_receiver(id, app_token.clone(), sender.clone()).await {
                continue;
            }
            else {
                // 異常終了した場合は5分経過した後に再接続を試行
                tokio::time::sleep(tokio::time::Duration::from_secs(360)).await;
            }
        }
    }

    // 4つのチャンネルでメッセージの受信を分散
    // Todo:
    // 1.  確立的に4つのチャンネル全てが同時に切断される可能性があるので、
    //     切断の警告を受信したタイミングで他のチャンネルを再起動するようにする
    // 2.  1つのタスクが終了したら全て終了するようにする
    //     (JoinHandleはjoinしなくてもいい説も確認)
    // 3.  エラー処理をちゃんと実装する
    async fn multi_websocket_receiver(app_token: String, sender: Sender<Message>) {
        let num_channels = 4;
        let mut tasks = Vec::new();
        for id in 0..num_channels {
            let task = tokio::spawn(auto_reconnecting(id, app_token.clone(), sender.clone()));
            tasks.push(task);
            tokio::time::sleep(tokio::time::Duration::from_millis(1000 * 360 / (num_channels + 1) as u64)).await;
        }
        future::join_all(tasks.into_iter()).await;
    }

    let (sender, mut receiver) = channel::<Message>(128);
    let _ = tokio::spawn(multi_websocket_receiver(app_token, sender.clone()));

    loop {
        let receive = receiver.try_next();
        if let Ok(Some(message)) = receive {
            message_handler(message);
        }
        else {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000 / 60 as u64)).await;
        }
    }
}

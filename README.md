# Slack Tower Battle
どうぶつタワーバトルのオマージュです。
slack上でゲームをプレイできます。

# Todo

- 処理が失敗した時に適切なエラーを返すようにする
- 終了処理をちゃんとする

# 遊び方
botにメンションを飛ばすとゲームが開始します。
ステージは24時間操作がないとリセットされます。

# 必要なスコープ

- `app_mentions:read`
- `chat:write`
- `files:write`
- `users.profile:read`

# ビルド & 実行

```bash
cd slack_tower_battle
echo "SLACK_APP_TOKEN=xapp-xxxxxxxx" > .env
echo "SLACK_BOT_TOKEN=xoxb-xxxxxxxx" >> .env
cargo run --release
```

トークンの部分は適宜書き換えて実行してください。

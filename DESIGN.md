# Sumika Console — 設計書

## 目的
あつやさんがローカルLLM（Strata 上の Qwen）を手動で簡単に「起動・設定・プロンプト入力（チャット）・終了」できる Windows デスクトップアプリ。単一 exe で配布する。

## 技術
- Rust + eframe/egui（GUI）、ureq（HTTP、rustls）、serde / serde_json
- Windows 専用。単一の release exe（依存 DLL なし）

## 重要な前提
- Strata: `%LOCALAPPDATA%\Strata\terminal\current\bin\strata.cmd`（.cmd なので起動は `cmd /C` 経由）
- サーバー: `http://127.0.0.1:<port>`（既定 8080）、OpenAI 互換 API（GET /v1/models は状態監視専用。チャットは v0.2.0 から OpenCode 経由＝次項）
- **日本語フォント必須**: egui 標準フォントには CJK が無い。起動時に `C:\Windows\Fonts\YuGothM.ttc`（無ければ `meiryob.ttc`、`msgothic.ttc` の順に探す）を読み込み、FontDefinitions の Proportional/Monospace の末尾にフォールバックとして追加すること。

## 画面構成（1ウィンドウ、タブ切替）
### 共通ヘッダー
- サーバー状態表示: ● 起動中（モデル名を /v1/models から表示）/ ○ 停止中 / ◐ 起動処理中
- 「起動」「終了」ボタン（状態に応じて活性/非活性）
- 状態は数秒ごとにポーリングして自動更新

### 「チャット」タブ（v0.2.0 で方式変更）
- チャットは彼女の本体（OpenCode）経由で送る：`opencode run --auto --format json --model strata/<起動中モデル id>` を作業フォルダ `D:\strata-work\friend` で実行し、憲章・記憶・道具を持った本人と会話する。OPENCODE_CONFIG には exe の隣に生成する `opencode-config.json`（strata プロバイダ＋モデル定義）を指定する
- 会話の継続は OpenCode のセッション（`--session`）を引き継ぐ。「新しい会話」ボタンで履歴とセッションをリセットする
- 会話履歴（スクロール表示、ユーザー/アシスタントを区別）
- 入力欄（複数行、lock_focus）＋「送信」ボタン（Ctrl+Enter でも送信）。送信中は「生成中…（経過秒）」表示。初回は本体側の文脈読み込みで数分かかることがある
- IME 変換中は Tab キーを egui に渡さない（MS-IME の予測変換が Tab を使うため。egui 0.29 の TextEdit は Tab をタブ文字挿入として処理し、変換を壊す）
- 実行はワーカースレッド＋ mpsc チャネルで UI をブロックしない。制限時間 30 分

### 「設定」タブ
- family: ドロップダウン（qwen / qwen36）
- model: テキスト（例: IQ3_XXS、UD-Q4_K_XL）
- context: テキスト（例: 128k）
- vision: ドロップダウン（off / on / gpu / cpu）
- parallel: 数値（1〜8）
- port: 数値（既定 8080）
- 追加引数: 複数行テキスト。`strata launch` のオプションとして末尾に付与される。通常は空欄。**起動プロファイルの `user_args` はエンジン層の引数であり、ここへ入れてはいけない**（入れると launch が即失敗する。user_args はプロファイル側に保存され起動時に自動適用される）
- 設定は exe と同じフォルダの `settings.json` に保存・読込（「保存」ボタン＋起動時に自動読込）

## 起動/終了の実装
- 起動: `cmd /C strata.cmd launch --family <f> --model <m> --context <c> --vision <v> --parallel <n> --port <p> --yes --no-tui <追加引数>` を DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP で spawn し、即座に制御を返す。成否はポーリング（/v1/models）で判定し、一定時間応答が無ければ「起動に時間がかかっています」と表示（エラー断定しない）
- 終了: `cmd /C strata.cmd launch --stop` を実行。ポーリングで停止を確認

## 自己テスト
- `--selftest` 引数で GUI を出さず: ① settings.json の保存→読込の往復一致、② サーバー起動中なら /v1/models の取得とモデル名表示、をコンソールに出力し、exit code（0=成功）で終了する

## ビルドと配置
- 開発は手元（VM）にソースを書き、PC の `C:\Users\Atsuya\source\sumika-console` へコピーして `cargo build --release`
- 成果物 `sumika-console.exe` を `C:\Users\Atsuya\tools\sumika-console\` へ配置
- 最終ソース一式を `~/workspace/projects/sumika-console/src/` に戻す（後で GitHub 公開に使う）
- README.md（日本語、使い方とビルド方法）を同梱する

#![windows_subsystem = "windows"]

mod api;
mod opencode;
mod settings;
mod strata;

use api::ChatMsg;
use eframe::egui;
use settings::Settings;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// 自己テスト（--selftest）
// ---------------------------------------------------------------------------

/// GUI アプリとして起動された場合でも、親コンソールへ出力できるようにする。
/// （SSH 経由などコンソールが無い環境では失敗するが、その場合は結果ファイルで確認する）
#[cfg(windows)]
fn attach_parent_console() {
    extern "system" {
        fn AttachConsole(dw_process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

fn run_selftest() -> i32 {
    attach_parent_console();
    let mut lines: Vec<String> = Vec::new();
    let path = settings::settings_path();
    let s1 = Settings::load(&path);
    let mut ok = true;
    match s1.save(&path) {
        Ok(()) => {
            let s2 = Settings::load(&path);
            if s1 == s2 {
                lines.push("[selftest] settings round-trip: OK".to_string());
            } else {
                ok = false;
                lines.push("[selftest] settings round-trip: MISMATCH".to_string());
            }
        }
        Err(e) => {
            ok = false;
            lines.push(format!("[selftest] settings save failed: {e}"));
        }
    }
    match api::fetch_models(s1.port) {
        Ok(ids) => {
            lines.push(format!(
                "[selftest] /v1/models OK: {} model(s): {}",
                ids.len(),
                ids.join(", ")
            ));
        }
        Err(e) => {
            lines.push(format!(
                "[selftest] /v1/models not reachable (skipped): {e}"
            ));
        }
    }
    lines.push(format!(
        "[selftest] result: {}",
        if ok { "OK" } else { "NG" }
    ));
    let text = lines.join("\n");
    println!("{text}");
    // コンソールに出ない環境でも確認できるよう、結果をファイルにも残す
    let result_path = path.with_file_name("selftest-result.txt");
    let _ = std::fs::write(&result_path, format!("{text}\n"));
    if ok {
        0
    } else {
        1
    }
}

// ---------------------------------------------------------------------------
// 日本語フォント
// ---------------------------------------------------------------------------

fn install_japanese_fonts(ctx: &egui::Context) {
    let candidates = [
        "C:\\Windows\\Fonts\\YuGothM.ttc",
        "C:\\Windows\\Fonts\\meiryob.ttc",
        "C:\\Windows\\Fonts\\msgothic.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("japanese".to_owned(), egui::FontData::from_owned(bytes));
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                fam.push("japanese".to_owned());
            }
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                fam.push("japanese".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// アプリ本体
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum ServerStatus {
    Stopped,
    Running(String), // 先頭のモデル id
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Chat,
    Settings,
}

struct ConsoleApp {
    settings: Settings,
    settings_path: PathBuf,
    tab: Tab,
    messages: Vec<ChatMsg>,
    input: String,
    generating: bool,
    chat_rx: Option<mpsc::Receiver<Result<(String, Option<String>), String>>>,
    /// OpenCode 側の会話セッション（継続用）。新しい会話で None に戻す。
    oc_session: Option<String>,
    /// 生成開始時刻（経過秒の表示用）
    chat_started: Option<Instant>,
    /// IME の変換（未確定文字列）が進行中かどうか。変換中は Tab と Enter を egui に渡さない。
    ime_composing: bool,
    status: ServerStatus,
    status_rx: mpsc::Receiver<(ServerStatus, Vec<String>)>,
    port_shared: Arc<Mutex<u16>>,
    starting_since: Option<Instant>,
    notice: String,
    saved_at: Option<Instant>,
    stop_rx: Option<mpsc::Receiver<Result<(), String>>>,
}

impl ConsoleApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_japanese_fonts(&cc.egui_ctx);
        let settings_path = settings::settings_path();
        let settings = Settings::load(&settings_path);
        let port_shared = Arc::new(Mutex::new(settings.port));
        let (tx, rx) = mpsc::channel::<(ServerStatus, Vec<String>)>();
        {
            let ps = Arc::clone(&port_shared);
            std::thread::spawn(move || loop {
                let port = *ps.lock().unwrap();
                let msg = match api::fetch_models(port) {
                    Ok(ids) => {
                        let first = ids.first().cloned().unwrap_or_default();
                        (ServerStatus::Running(first), ids)
                    }
                    Err(_) => (ServerStatus::Stopped, Vec::new()),
                };
                if tx.send(msg).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_secs(3));
            });
        }
        Self {
            settings,
            settings_path,
            tab: Tab::Chat,
            messages: Vec::new(),
            input: String::new(),
            generating: false,
            chat_rx: None,
            oc_session: None,
            chat_started: None,
            ime_composing: false,
            status: ServerStatus::Stopped,
            status_rx: rx,
            port_shared,
            starting_since: None,
            notice: String::new(),
            saved_at: None,
            stop_rx: None,
        }
    }

    fn current_model(&self) -> Option<String> {
        match &self.status {
            ServerStatus::Running(id) if !id.is_empty() => Some(id.clone()),
            _ => None,
        }
    }

    fn send_chat(&mut self) {
        if self.generating {
            return;
        }
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        let Some(model) = self.current_model() else {
            self.notice =
                "サーバーが起動していません。先に「起動」を押してください。".to_string();
            return;
        };
        self.input.clear();
        self.messages.push(ChatMsg {
            role: "user".to_string(),
            content: text.clone(),
        });
        // チャットは彼女の本体（OpenCode）経由で送る。人格・記憶・道具は
        // 本体側（憲章）で定義されるため、ここでは会話履歴を再送しない。
        // 継続は OpenCode のセッションを引き継ぐことで行う。
        let port = self.settings.port;
        let session = self.oc_session.clone();
        let (tx, rx) = mpsc::channel();
        self.chat_rx = Some(rx);
        self.generating = true;
        self.chat_started = Some(Instant::now());
        std::thread::spawn(move || {
            let r = opencode::run_chat(&model, port, &session, &text);
            let _ = tx.send(r);
        });
    }

    fn new_conversation(&mut self) {
        if self.generating {
            return;
        }
        self.messages.clear();
        self.oc_session = None;
        self.notice = "新しい会話を始めました。".to_string();
    }

    fn chat_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("チャットは彼女の本体（OpenCode）経由です。人格・記憶・道具は本体側の設定がそのまま使われます。");
        ui.separator();

        let input_height = 118.0;
        let avail = ui.available_height();
        egui::ScrollArea::vertical()
            .max_height((avail - input_height).max(80.0))
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.messages.is_empty() {
                    ui.label("（まだ会話がありません）");
                }
                for m in &self.messages {
                    if m.role == "user" {
                        ui.label(
                            egui::RichText::new("あなた")
                                .strong()
                                .color(egui::Color32::from_rgb(0x1f, 0x6f, 0xeb)),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("すみか")
                                .strong()
                                .color(egui::Color32::from_rgb(0xbc, 0x4c, 0x00)),
                        );
                    }
                    ui.label(&m.content);
                    ui.add_space(6.0);
                }
                if self.generating {
                    let secs = self
                        .chat_started
                        .map(|t| t.elapsed().as_secs())
                        .unwrap_or(0);
                    ui.label(format!("生成中…（{secs}秒）"));
                }
            });
        ui.separator();

        let input_resp = ui.add(
            egui::TextEdit::multiline(&mut self.input)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                // Tab でフォーカスを手放さない（IME変換中にフォーカスが外れると変換が消えるため）
                .lock_focus(true)
                .hint_text("メッセージを入力"),
        );
        ui.horizontal(|ui| {
            if matches!(self.status, ServerStatus::Stopped) && self.starting_since.is_none() {
                ui.label("サーバー停止中は送信できません");
            }
            if ui
                .add_enabled(!self.generating, egui::Button::new("新しい会話"))
                .clicked()
            {
                self.new_conversation();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_send =
                    !self.generating && matches!(self.status, ServerStatus::Running(_));
                if ui.add_enabled(can_send, egui::Button::new("送信")).clicked() {
                    self.send_chat();
                }
                // 送信ボタンと並べて常時表示する（入力欄のヒント文は打ち始めると消えるため）
                ui.label("Ctrl+Enterで送信");
            });
        });
        let ctrl_enter = input_resp.has_focus()
            && ui
                .ctx()
                .input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl);
        if ctrl_enter {
            self.send_chat();
        }
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("family");
                egui::ComboBox::from_id_source("family_cb")
                    .selected_text(&self.settings.family)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings.family,
                            "qwen".to_string(),
                            "qwen",
                        );
                        ui.selectable_value(
                            &mut self.settings.family,
                            "qwen36".to_string(),
                            "qwen36",
                        );
                    });
                ui.end_row();

                ui.label("model");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.model).desired_width(240.0),
                );
                ui.end_row();

                ui.label("context");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.context)
                        .desired_width(240.0),
                );
                ui.end_row();

                ui.label("vision");
                egui::ComboBox::from_id_source("vision_cb")
                    .selected_text(&self.settings.vision)
                    .show_ui(ui, |ui| {
                        for v in ["off", "on", "gpu", "cpu"] {
                            ui.selectable_value(
                                &mut self.settings.vision,
                                v.to_string(),
                                v,
                            );
                        }
                    });
                ui.end_row();

                ui.label("parallel");
                ui.add(egui::DragValue::new(&mut self.settings.parallel).speed(0.2));
                ui.end_row();

                ui.label("port");
                ui.add(egui::DragValue::new(&mut self.settings.port).speed(1.0));
                ui.end_row();
            });
        self.settings.parallel = self.settings.parallel.clamp(1, 8);
        if self.settings.port == 0 {
            self.settings.port = 8080;
        }

        ui.add_space(8.0);
        ui.label("追加引数（strata launch のオプションとして末尾に付与。通常は空欄で構いません）");
        ui.add(
            egui::TextEdit::multiline(&mut self.settings.extra_args)
                .desired_rows(5)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("保存").clicked() {
                match self.settings.save(&self.settings_path) {
                    Ok(()) => {
                        *self.port_shared.lock().unwrap() = self.settings.port;
                        self.saved_at = Some(Instant::now());
                    }
                    Err(e) => {
                        self.notice = format!("設定の保存に失敗しました: {e}");
                    }
                }
            }
            if let Some(t) = self.saved_at {
                if t.elapsed().as_secs() < 3 {
                    ui.label("保存しました");
                }
            }
        });
        ui.label(format!(
            "設定ファイル: {}",
            self.settings_path.display()
        ));
        ui.label("設定の変更は、次回の起動から反映されます。");
    }
}

impl eframe::App for ConsoleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // IME変換中は Tab と Enter を egui に渡さない。
        // MS-IME の予測変換は Tab で候補を選ぶが、egui の TextEdit は Tab を
        // タブ文字の挿入として先に処理してしまい、変換が確定できないため。
        // 同様に、変換の確定に使う Enter も TextEdit が改行として処理してしまう。
        // （フォーカス移動は入力欄の lock_focus(true) 側で止めている）
        {
            // 確定（Commit）が Enter と同じフレームで届くと composing が先に
            // false へ倒れて Enter が漏れるため、前フレームの状態でも止める。
            let was_composing = self.ime_composing;
            let mut composing = was_composing;
            ctx.input(|i| {
                for e in &i.events {
                    if let egui::Event::Ime(ie) = e {
                        match ie {
                            egui::ImeEvent::Preedit(t) => composing = !t.is_empty(),
                            egui::ImeEvent::Commit(_)
                            | egui::ImeEvent::Disabled => composing = false,
                            _ => {}
                        }
                    }
                }
            });
            self.ime_composing = composing;
            if was_composing || composing {
                ctx.input_mut(|i| {
                    i.events.retain(|e| {
                        !matches!(
                            e,
                            egui::Event::Key {
                                key: egui::Key::Tab | egui::Key::Enter,
                                ..
                            }
                        )
                    });
                });
            }
        }
        // ポーリング結果の取り込み
        while let Ok((st, _models)) = self.status_rx.try_recv() {
            if matches!(st, ServerStatus::Running(_)) {
                self.starting_since = None;
            }
            self.status = st;
        }
        // チャット結果の取り込み
        if let Some(rx) = &self.chat_rx {
            match rx.try_recv() {
                Ok(Ok((text, session))) => {
                    self.messages.push(ChatMsg {
                        role: "assistant".to_string(),
                        content: text,
                    });
                    if session.is_some() {
                        self.oc_session = session;
                    }
                    self.generating = false;
                    self.chat_started = None;
                    self.chat_rx = None;
                }
                Ok(Err(e)) => {
                    self.messages.push(ChatMsg {
                        role: "assistant".to_string(),
                        content: format!("⚠ エラー: {e}"),
                    });
                    self.generating = false;
                    self.chat_started = None;
                    self.chat_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.generating = false;
                    self.chat_started = None;
                    self.chat_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        // 終了コマンド結果の取り込み
        if let Some(rx) = &self.stop_rx {
            match rx.try_recv() {
                Ok(Ok(())) => {
                    self.notice = "終了コマンドを送りました。".to_string();
                    self.stop_rx = None;
                }
                Ok(Err(e)) => {
                    self.notice = format!("終了に失敗しました: {e}");
                    self.stop_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.stop_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let (icon, color, label) = match &self.status {
                    ServerStatus::Running(id) => (
                        "●",
                        egui::Color32::from_rgb(0x2e, 0xa0, 0x43),
                        format!("起動中（{id}）"),
                    ),
                    ServerStatus::Stopped => {
                        if let Some(since) = self.starting_since {
                            let secs = since.elapsed().as_secs();
                            (
                                "◐",
                                egui::Color32::from_rgb(0xd0, 0x8a, 0x00),
                                format!("起動処理中（{secs}秒）"),
                            )
                        } else {
                            ("○", egui::Color32::GRAY, "停止中".to_string())
                        }
                    }
                };
                ui.label(egui::RichText::new(icon).color(color).size(16.0));
                ui.label(egui::RichText::new(label).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let running = matches!(self.status, ServerStatus::Running(_));
                    let can_stop = running || self.starting_since.is_some();
                    if ui.add_enabled(can_stop, egui::Button::new("終了")).clicked() {
                        self.starting_since = None;
                        let (tx, rx) = mpsc::channel();
                        self.stop_rx = Some(rx);
                        std::thread::spawn(move || {
                            let _ = tx.send(strata::stop_server());
                        });
                    }
                    let can_start = !running && self.starting_since.is_none();
                    if ui.add_enabled(can_start, egui::Button::new("起動")).clicked() {
                        match strata::start_server(&self.settings) {
                            Ok(()) => {
                                self.starting_since = Some(Instant::now());
                                self.notice = "起動コマンドを送りました。モデルの読み込みには数分かかることがあります。".to_string();
                            }
                            Err(e) => {
                                self.notice = format!("起動に失敗しました: {e}");
                            }
                        }
                    }
                });
            });
            if let Some(since) = self.starting_since {
                if since.elapsed().as_secs() > 120
                    && matches!(self.status, ServerStatus::Stopped)
                {
                    ui.label(
                        "起動に時間がかかっています（モデルのダウンロード／読み込み中の可能性があります）。",
                    );
                }
            }
            if !self.notice.is_empty() {
                ui.label(&self.notice);
            }
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.tab == Tab::Chat, " チャット ")
                    .clicked()
                {
                    self.tab = Tab::Chat;
                }
                if ui
                    .selectable_label(self.tab == Tab::Settings, " 設定 ")
                    .clicked()
                {
                    self.tab = Tab::Settings;
                }
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Chat => self.chat_ui(ui),
            Tab::Settings => self.settings_ui(ui),
        });

        ctx.request_repaint_after(Duration::from_millis(600));
    }
}

fn main() {
    if std::env::args().any(|a| a == "--selftest") {
        std::process::exit(run_selftest());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Sumika Console")
            .with_inner_size([920.0, 640.0])
            .with_min_inner_size([680.0, 460.0]),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "Sumika Console",
        options,
        Box::new(|cc| Ok(Box::new(ConsoleApp::new(cc)))),
    ) {
        eprintln!("GUI の起動に失敗しました: {e}");
    }
}

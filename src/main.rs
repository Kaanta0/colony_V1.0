mod scan;
mod sections;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use iced::font::{self, Weight};
use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Column, Row,
};
use iced::{color, Element, Fill, Font, Length, Task, Theme};
use rand::distr::Alphanumeric;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tiny_http::Response;
use url::Url;

use scan::Application;
use sections::Section;

pub fn main() -> iced::Result {
    let font_assets = FontAssets::discover();
    let default_font = font_assets.default_font();

    iced::application(
        move || App::boot(font_assets.clone()),
        App::update,
        App::view,
    )
        .title(App::title)
        .theme(App::theme)
        .default_font(default_font)
        .window_size((1000.0, 700.0))
        .run()
}

const APP_FONT_NAME: &str = "JetBrainsMono Nerd Font";
const APP_FONT_FILES: [&str; 3] = [
    "JetBrainsMonoNerdFont-Regular.ttf",
    "JetBrainsMonoNerdFont-Medium.ttf",
    "JetBrainsMonoNerdFont-Bold.ttf",
];
const OAUTH_PORT: u16 = 8787;

#[derive(Clone)]
struct FontAssets {
    bytes: Vec<Vec<u8>>,
    complete: bool,
}

impl FontAssets {
    fn discover() -> Self {
        let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/ui/assets/fonts/JetBrainsMonoNerdFont");
        let mut bytes = Vec::new();
        let mut complete = true;

        for font_file in APP_FONT_FILES {
            let font_path = assets_dir.join(font_file);
            match std::fs::read(&font_path) {
                Ok(data) => bytes.push(data),
                Err(error) => {
                    complete = false;
                    eprintln!(
                        "Font asset missing or unreadable: {} ({error})",
                        font_path.display()
                    );
                }
            }
        }

        Self { bytes, complete }
    }

    fn default_font(&self) -> Font {
        if self.complete {
            Font::with_name(APP_FONT_NAME)
        } else {
            Font::MONOSPACE
        }
    }

    fn load_task(&self) -> Task<Message> {
        if !self.complete {
            return Task::none();
        }

        Task::batch(
            self.bytes
                .iter()
                .cloned()
                .map(|data| font::load(data).map(Message::FontLoaded)),
        )
    }
}

struct App {
    applications: Vec<Application>,
    dummy_apps: Vec<DummyApp>,
    search_query: String,
    sections: Vec<Section>,
    selected_section: usize,
    status_message: String,
    active_dummy_app: Option<usize>,
    font: Font,
    auth: AuthState,
}

#[derive(Clone)]
struct DummyApp {
    name: String,
    description: String,
    language: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AuthStatus {
    Connected,
    Disconnected,
    Connecting,
}

#[derive(Clone, Debug)]
struct AuthState {
    status: AuthStatus,
    message: String,
    token: Option<String>,
    expected_state: Option<String>,
    code_verifier: Option<String>,
}

impl AuthState {
    fn load() -> Self {
        match load_token() {
            Ok(Some(token)) => Self {
                status: AuthStatus::Connected,
                message: "Connecté à GitHub.".to_string(),
                token: Some(token),
                expected_state: None,
                code_verifier: None,
            },
            Ok(None) => Self {
                status: AuthStatus::Disconnected,
                message: "Non connecté.".to_string(),
                token: None,
                expected_state: None,
                code_verifier: None,
            },
            Err(message) => Self {
                status: AuthStatus::Disconnected,
                message: format!("Non connecté ({message})."),
                token: None,
                expected_state: None,
                code_verifier: None,
            },
        }
    }

    fn status_color(&self) -> iced::Color {
        match self.status {
            AuthStatus::Connected => color!(0x6bd68f),
            AuthStatus::Connecting => color!(0xf0c674),
            AuthStatus::Disconnected => color!(0x8a8aa3),
        }
    }
}

#[derive(Clone, Debug)]
struct OAuthConfig {
    client_id: String,
    client_secret: Option<String>,
}

impl OAuthConfig {
    fn from_env() -> Result<Self, String> {
        let client_id = std::env::var("GITHUB_CLIENT_ID")
            .map_err(|_| "Définissez GITHUB_CLIENT_ID pour OAuth GitHub.".to_string())?;
        let client_secret = std::env::var("GITHUB_CLIENT_SECRET").ok();
        Ok(Self {
            client_id,
            client_secret,
        })
    }
}

#[derive(Clone, Debug)]
struct OAuthRequest {
    auth_url: String,
    state: String,
    code_verifier: String,
    redirect_uri: String,
}

impl OAuthRequest {
    fn new(config: &OAuthConfig) -> Result<Self, String> {
        let code_verifier = generate_code_verifier();
        let code_challenge = generate_code_challenge(&code_verifier)?;
        let state = generate_state();
        let redirect_uri = format!("http://127.0.0.1:{OAUTH_PORT}/oauth/callback");

        let auth_url = format!(
            "https://github.com/login/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&scope=read:user&state={state}&code_challenge={code_challenge}&code_challenge_method=S256",
            client_id = config.client_id,
            redirect_uri = urlencoding::encode(&redirect_uri),
            state = state,
            code_challenge = code_challenge
        );

        Ok(Self {
            auth_url,
            state,
            code_verifier,
            redirect_uri,
        })
    }
}

impl App {
    fn boot(font_assets: FontAssets) -> (Self, Task<Message>) {
        let applications = scan::scan_applications().unwrap_or_else(|e| {
            eprintln!("Scan error: {e}");
            Vec::new()
        });

        let status_message = format!("{} applications found", applications.len());

        let sections = sections::load_sections();
        let dummy_apps = Self::dummy_development_apps();

        let font = font_assets.default_font();
        let auth = AuthState::load();

        let app = Self {
            applications,
            dummy_apps,
            search_query: String::new(),
            sections,
            selected_section: 0,
            status_message,
            active_dummy_app: None,
            font,
            auth,
        };

        (app, font_assets.load_task())
    }
}

#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    SectionSelected(usize),
    Rescan,
    LaunchApp(String),
    DummyAppSelected(usize),
    DummyAppBack,
    ClearStatus,
    FontLoaded(Result<(), font::Error>),
    LoginRequested,
    LoginUrlOpened(Result<(), String>),
    OAuthFinished(Result<String, String>),
}

impl App {
    fn title(&self) -> String {
        String::from("Colony Launcher")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SearchChanged(query) => {
                self.search_query = query;
                Task::none()
            }
            Message::SectionSelected(index) => {
                if index < self.sections.len() {
                    self.selected_section = index;
                    self.active_dummy_app = None;
                }
                Task::none()
            }
            Message::Rescan => {
                match scan::scan_applications() {
                    Ok(apps) => {
                        self.status_message = format!("{} applications found", apps.len());
                        self.applications = apps;
                    }
                    Err(e) => {
                        self.status_message = format!("Error: {e}");
                    }
                }
                Task::none()
            }
            Message::LaunchApp(exec) => {
                let launch_result = {
                    #[cfg(windows)]
                    {
                        // On Windows, use cmd /C start to launch
                        std::process::Command::new("cmd")
                            .args(["/C", "start", "", &exec])
                            .spawn()
                            .map(|_| ())
                            .map_err(|error| format!("Impossible de lancer: {error}"))
                    }

                    #[cfg(not(windows))]
                    {
                        // On Linux/macOS, parse and execute
                        match shell_words::split(&exec) {
                            Ok(mut parts) => {
                                parts.retain(|part| !part.is_empty());
                                if let Some((cmd, args)) = parts.split_first() {
                                    std::process::Command::new(cmd)
                                        .args(args)
                                        .spawn()
                                        .map(|_| ())
                                        .map_err(|error| {
                                            format!("Impossible de lancer: {error}")
                                        })
                                } else {
                                    Err("Impossible de lancer: commande vide".to_string())
                                }
                            }
                            Err(error) => Err(format!("Impossible de lancer: {error}")),
                        }
                    }
                };

                match launch_result {
                    Ok(()) => {
                        self.status_message = "Application lancée.".to_string();
                        return Task::perform(
                            async {
                                std::thread::sleep(Duration::from_secs(4));
                            },
                            |_| Message::ClearStatus,
                        );
                    }
                    Err(message) => {
                        self.status_message = message;
                    }
                }
                Task::none()
            }
            Message::DummyAppSelected(index) => {
                if index < self.dummy_apps.len() {
                    self.active_dummy_app = Some(index);
                }
                Task::none()
            }
            Message::DummyAppBack => {
                self.active_dummy_app = None;
                Task::none()
            }
            Message::ClearStatus => {
                self.status_message = format!("{} applications found", self.applications.len());
                Task::none()
            }
            Message::FontLoaded(_) => Task::none(),
            Message::LoginRequested => {
                if self.auth.status == AuthStatus::Connecting {
                    return Task::none();
                }

                let config = match OAuthConfig::from_env() {
                    Ok(config) => config,
                    Err(message) => {
                        self.auth.status = AuthStatus::Disconnected;
                        self.auth.message = message;
                        return Task::none();
                    }
                };

                let oauth_request = match OAuthRequest::new(&config) {
                    Ok(request) => request,
                    Err(message) => {
                        self.auth.status = AuthStatus::Disconnected;
                        self.auth.message = message;
                        return Task::none();
                    }
                };

                self.auth.status = AuthStatus::Connecting;
                self.auth.message =
                    "Ouverture du navigateur pour se connecter à GitHub...".to_string();
                self.auth.expected_state = Some(oauth_request.state.clone());
                self.auth.code_verifier = Some(oauth_request.code_verifier.clone());

                let open_url = oauth_request.auth_url.clone();
                let redirect_uri = oauth_request.redirect_uri.clone();
                let state = oauth_request.state.clone();
                let code_verifier = oauth_request.code_verifier.clone();

                Task::batch([
                    Task::perform(
                        async move { open_browser(&open_url) },
                        Message::LoginUrlOpened,
                    ),
                    Task::perform(
                        async move {
                            oauth_flow(state, code_verifier, redirect_uri, config)
                        },
                        Message::OAuthFinished,
                    ),
                ])
            }
            Message::LoginUrlOpened(result) => {
                if let Err(message) = result {
                    self.auth.status = AuthStatus::Disconnected;
                    self.auth.message = message;
                    self.auth.expected_state = None;
                    self.auth.code_verifier = None;
                }
                Task::none()
            }
            Message::OAuthFinished(result) => {
                match result {
                    Ok(token) => {
                        self.auth.status = AuthStatus::Connected;
                        self.auth.token = Some(token.clone());
                        self.auth.message = "Connecté à GitHub.".to_string();
                        if let Err(message) = save_token(&token) {
                            self.auth.message = format!(
                                "Connecté, mais impossible de sauvegarder le token: {message}"
                            );
                        }
                    }
                    Err(message) => {
                        self.auth.status = AuthStatus::Disconnected;
                        self.auth.message = message;
                    }
                }
                self.auth.expected_state = None;
                self.auth.code_verifier = None;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = self.view_content();

        let main_layout = row![sidebar, content].spacing(0);

        container(main_layout)
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let title = text("Colony")
            .size(30)
            .font(self.app_font_with_weight(Weight::Bold));

        let category_header = text("Catégories")
            .size(13)
            .font(self.app_font())
            .color(color!(0x8a8aa3));

        let category_buttons: Vec<Element<'_, Message>> = self
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| self.view_section_button(index, section))
            .collect();

        let category_list = Column::with_children(category_buttons).spacing(4);
        let category_scroll = scrollable(category_list).height(Length::Fill);

        let rescan_btn = button(text("Rescan").size(13).font(self.app_font()))
            .on_press(Message::Rescan)
            .padding([8, 16])
            .width(Fill);

        let sidebar_content = column![
            title,
            container(text("")).height(24),
            category_header,
            category_scroll,
            container(text("")).height(Length::Fill),
            rescan_btn,
        ]
        .spacing(10)
        .padding(16)
        .width(200);

        container(sidebar_content)
            .style(|_theme| container::Style {
                background: Some(color!(0x1a1a2e).into()),
                ..Default::default()
            })
            .height(Fill)
            .into()
    }

    fn view_section_button(&self, index: usize, section: &Section) -> Element<'_, Message> {
        let is_selected = self.selected_section == index;

        let text_color = if is_selected {
            color!(0xffffff)
        } else {
            color!(0x9a9ab5)
        };

        let indicator = container(text(""))
            .width(4)
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(
                    if is_selected {
                        color!(0x6b6bd6)
                    } else {
                        iced::Color::TRANSPARENT
                    }
                    .into(),
                ),
                ..Default::default()
            });

        let icon = text(section.icon.clone())
            .size(15)
            .font(self.app_font())
            .color(text_color);

        let label = text(section.name.clone())
            .size(14)
            .font(self.app_font())
            .color(text_color);

        let content = row![indicator, icon, label]
            .spacing(10)
            .align_y(iced::Alignment::Center);

        let btn = button(content)
            .on_press(Message::SectionSelected(index))
            .padding([10, 14])
            .width(Fill)
            .style(move |theme, status| {
                if is_selected {
                    button::Style {
                        background: Some(color!(0x3a3a5e).into()),
                        text_color: color!(0xffffff),
                        border: iced::Border::default().rounded(6),
                        ..button::primary(theme, status)
                    }
                } else {
                    button::Style {
                        background: Some(iced::Color::TRANSPARENT.into()),
                        text_color: color!(0x888899),
                        border: iced::Border::default().rounded(6),
                        ..button::secondary(theme, status)
                    }
                }
            });

        btn.into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        if let Some(index) = self.active_dummy_app {
            if let Some(app) = self.dummy_apps.get(index) {
                return self.view_dummy_detail(app);
            }
        }

        let search = text_input("Search applications...", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(12)
            .size(16)
            .width(Fill);

        let status = text(&self.status_message)
            .size(12)
            .font(self.app_font())
            .color(color!(0x888899));

        let auth_feedback = text(&self.auth.message)
            .size(12)
            .font(self.app_font())
            .color(self.auth.status_color());

        let login_button = {
            let button_base = button(text("Se connecter").size(13).font(self.app_font()))
                .padding([8, 16]);
            if self.auth.status == AuthStatus::Connected
                || self.auth.status == AuthStatus::Connecting
            {
                button_base
            } else {
                button_base.on_press(Message::LoginRequested)
            }
        };

        let auth_column = column![login_button, auth_feedback].spacing(4);

        let header = row![search, status, auth_column]
            .spacing(16)
            .align_y(iced::Alignment::Center);

        let app_grid = self.view_app_grid();

        let content = column![
            header,
            container(text("")).height(16),
            app_grid
        ]
        .spacing(8)
        .padding(24)
        .width(Fill);

        container(content)
            .style(|_theme| container::Style {
                background: Some(color!(0x0f0f1a).into()),
                ..Default::default()
            })
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn view_app_grid(&self) -> Element<'_, Message> {
        if self.is_development_section() {
            return self.view_dummy_grid();
        }

        let filtered: Vec<&Application> = self.filtered_applications();

        if filtered.is_empty() {
            return container(
                text("No applications found")
                    .size(16)
                    .color(color!(0x666677)),
            )
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into();
        }

        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        for chunk in filtered.chunks(4) {
            let mut row_items: Vec<Element<'_, Message>> = Vec::new();

            for app in chunk {
                row_items.push(self.view_app_card(app));
            }

            while row_items.len() < 4 {
                row_items.push(container(column![]).width(Fill).into());
            }

            rows.push(Row::with_children(row_items).spacing(12).into());
        }

        let grid = Column::with_children(rows).spacing(12);

        scrollable(grid).height(Fill).into()
    }

    fn view_app_card(&self, app: &Application) -> Element<'_, Message> {
        let icon_char = app.name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?');

        let icon = text(icon_char.to_string())
            .size(32)
            .font(self.app_font_with_weight(Weight::Medium))
            .color(color!(0x8888ff));

        let name = text(app.name.clone())
            .size(14)
            .font(self.app_font())
            .color(color!(0xffffff));

        let card_content = column![
            container(icon)
                .width(Fill)
                .center_x(Fill),
            container(text("")).height(8),
            container(name)
                .width(Fill)
                .height(32)
                .center_x(Fill)
                .center_y(Fill),
        ]
        .spacing(4)
        .padding(16)
        .width(Fill);

        let exec = app.exec.clone();
        button(card_content)
            .on_press(Message::LaunchApp(exec))
            .padding(0)
            .width(Fill)
            .height(120)
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered => color!(0x2a2a4e),
                    button::Status::Pressed => color!(0x3a3a5e),
                    _ => color!(0x1a1a2e),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: color!(0xffffff),
                    border: iced::Border::default().rounded(12),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_dummy_grid(&self) -> Element<'_, Message> {
        let filtered = self.filtered_dummy_apps();

        if filtered.is_empty() {
            return container(
                text("No applications found")
                    .size(16)
                    .color(color!(0x666677)),
            )
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into();
        }

        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        for chunk in filtered.chunks(4) {
            let mut row_items: Vec<Element<'_, Message>> = Vec::new();

            for (index, app) in chunk.iter() {
                row_items.push(self.view_dummy_card(*index, *app));
            }

            while row_items.len() < 4 {
                row_items.push(container(column![]).width(Fill).into());
            }

            rows.push(Row::with_children(row_items).spacing(12).into());
        }

        let grid = Column::with_children(rows).spacing(12);

        scrollable(grid).height(Fill).into()
    }

    fn view_dummy_card(&self, index: usize, app: &DummyApp) -> Element<'_, Message> {
        let icon_char = app.name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?');

        let icon = text(icon_char.to_string())
            .size(32)
            .font(self.app_font_with_weight(Weight::Medium))
            .color(color!(0x8888ff));

        let name = text(app.name.clone())
            .size(14)
            .font(self.app_font())
            .color(color!(0xffffff));

        let card_content = column![
            container(icon)
                .width(Fill)
                .center_x(Fill),
            container(text("")).height(8),
            container(name)
                .width(Fill)
                .height(32)
                .center_x(Fill)
                .center_y(Fill),
        ]
        .spacing(4)
        .padding(16)
        .width(Fill);

        button(card_content)
            .on_press(Message::DummyAppSelected(index))
            .padding(0)
            .width(Fill)
            .height(120)
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered => color!(0x2a2a4e),
                    button::Status::Pressed => color!(0x3a3a5e),
                    _ => color!(0x1a1a2e),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: color!(0xffffff),
                    border: iced::Border::default().rounded(12),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_dummy_detail<'a>(&self, app: &'a DummyApp) -> Element<'a, Message> {
        let back_button = button(text("Retour").size(13).font(self.app_font()))
            .on_press(Message::DummyAppBack)
            .padding([8, 16]);

        let title = text(&app.name)
            .size(24)
            .font(self.app_font_with_weight(Weight::Bold))
            .color(color!(0xffffff));

        let description = text(&app.description)
            .size(16)
            .font(self.app_font())
            .color(color!(0xcfcfe6));

        let language = text(format!("Langage: {}", app.language))
            .size(12)
            .font(self.app_font())
            .color(color!(0x8a8aa3));

        let header = row![back_button]
            .width(Fill)
            .align_y(iced::Alignment::Center);

        let body = container(description)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill);

        let footer = row![container(text("")).width(Fill), language]
            .align_y(iced::Alignment::End);

        let detail = column![
            header,
            container(title).width(Fill).center_x(Fill),
            body,
            footer
        ]
        .spacing(16)
        .padding(24)
        .width(Fill)
        .height(Fill);

        container(detail)
            .style(|_theme| container::Style {
                background: Some(color!(0x0f0f1a).into()),
                ..Default::default()
            })
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn filtered_applications(&self) -> Vec<&Application> {
        let query = self.search_query.to_lowercase();
        let selected_section = self.sections.get(self.selected_section);
        self.applications
            .iter()
            .filter(|app| {
                if let Some(section) = selected_section {
                    if !section.filter.matches(app) {
                        return false;
                    }
                }
                // Search filter
                if query.is_empty() {
                    return true;
                }
                app.name.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn filtered_dummy_apps(&self) -> Vec<(usize, &DummyApp)> {
        let query = self.search_query.to_lowercase();
        self.dummy_apps
            .iter()
            .enumerate()
            .filter(|(_, app)| {
                if query.is_empty() {
                    true
                } else {
                    app.name.to_lowercase().contains(&query)
                }
            })
            .collect()
    }

    fn is_development_section(&self) -> bool {
        self.sections
            .get(self.selected_section)
            .and_then(|section| section.category())
            .map(|category| category == &scan::AppCategory::Development)
            .unwrap_or(false)
    }

    fn dummy_development_apps() -> Vec<DummyApp> {
        vec![
            DummyApp {
                name: "Swift Toolkit".to_string(),
                description: "Boîte à outils moderne pour prototyper des apps Swift et explorer les API Apple."
                    .to_string(),
                language: "Swift".to_string(),
            },
            DummyApp {
                name: "Kotlin Studio".to_string(),
                description: "Espace de travail rapide pour composer, tester et profiler du code Kotlin."
                    .to_string(),
                language: "Kotlin".to_string(),
            },
            DummyApp {
                name: "Dart Lab".to_string(),
                description: "Laboratoire expérimental pour itérer sur des widgets et des scripts Dart."
                    .to_string(),
                language: "Dart".to_string(),
            },
            DummyApp {
                name: "TypeScript Playground".to_string(),
                description:
                    "Environnement interactif pour explorer les types, les utilitaires et les bibliothèques TS."
                        .to_string(),
                language: "TypeScript".to_string(),
            },
            DummyApp {
                name: "Flutter Dev Tools".to_string(),
                description:
                    "Console de performance pour déboguer des interfaces Flutter et leurs animations."
                        .to_string(),
                language: "Dart / Flutter".to_string(),
            },
        ]
    }

    fn app_font(&self) -> Font {
        self.font
    }

    fn app_font_with_weight(&self, weight: Weight) -> Font {
        Font { weight, ..self.font }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

#[derive(Debug, serde::Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StoredToken {
    access_token: String,
}

fn open_browser(url: &str) -> Result<(), String> {
    webbrowser::open(url).map(|_| ()).map_err(|error| error.to_string())
}

fn oauth_flow(
    expected_state: String,
    code_verifier: String,
    redirect_uri: String,
    config: OAuthConfig,
) -> Result<String, String> {
    let code = wait_for_oauth_callback(&expected_state)?;
    exchange_code_for_token(&config, &code, &code_verifier, &redirect_uri)
}

fn wait_for_oauth_callback(expected_state: &str) -> Result<String, String> {
    let server = tiny_http::Server::http(("127.0.0.1", OAUTH_PORT))
        .map_err(|error| format!("Impossible de démarrer le serveur OAuth: {error}"))?;

    let request = server
        .recv()
        .map_err(|error| format!("Erreur lors de la réception OAuth: {error}"))?;

    let request_url = format!("http://localhost{}", request.url());
    let parsed = Url::parse(&request_url)
        .map_err(|error| format!("URL OAuth invalide: {error}"))?;

    let mut code = None;
    let mut state = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            _ => {}
        }
    }

    let response = Response::from_string(
        "<html><body>Connexion réussie. Vous pouvez fermer cette fenêtre.</body></html>",
    );
    let _ = request.respond(response);

    let Some(code) = code else {
        return Err("Le callback OAuth ne contient pas de code.".to_string());
    };

    if state.as_deref() != Some(expected_state) {
        return Err("État OAuth invalide.".to_string());
    }

    Ok(code)
}

fn exchange_code_for_token(
    config: &OAuthConfig,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    let request = ureq::post("https://github.com/login/oauth/access_token")
        .set("Accept", "application/json");

    let mut form = vec![
        ("client_id", config.client_id.as_str()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
    ];

    if let Some(secret) = config.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }

    let response = request
        .send_form(&form)
        .map_err(|error| format!("Erreur OAuth: {error}"))?;

    let token_response: OAuthTokenResponse = response
        .into_json()
        .map_err(|error| format!("Réponse OAuth invalide: {error}"))?;

    Ok(token_response.access_token)
}

fn generate_code_verifier() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

fn generate_code_challenge(code_verifier: &str) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    Ok(URL_SAFE_NO_PAD.encode(hash))
}

fn generate_state() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn token_storage_path() -> Result<PathBuf, String> {
    let base_dir = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Impossible de trouver un dossier pour stocker le token.".to_string())?;
    Ok(base_dir.join("colony").join("github_token.json"))
}

fn save_token(token: &str) -> Result<(), String> {
    let path = token_storage_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Impossible de créer le dossier token: {error}"))?;
    }

    let payload = StoredToken {
        access_token: token.to_string(),
    };
    let data = serde_json::to_vec_pretty(&payload)
        .map_err(|error| format!("Impossible de sérialiser le token: {error}"))?;
    std::fs::write(&path, data)
        .map_err(|error| format!("Impossible d'écrire le token: {error}"))?;
    Ok(())
}

fn load_token() -> Result<Option<String>, String> {
    let path = token_storage_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read(&path)
        .map_err(|error| format!("Impossible de lire le token: {error}"))?;
    let payload: StoredToken = serde_json::from_slice(&data)
        .map_err(|error| format!("Impossible de parser le token: {error}"))?;
    Ok(Some(payload.access_token))
}

mod colony_apps;
mod github;
mod manifest;
mod scan;
mod sections;

use iced::font::{self, Weight};
use iced::widget::{Column, Row, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill, Font, Length, Task, Theme, color};
use std::path::Path;
use std::time::Duration;

use github::ColonySoftware;
use scan::{AppOrigin, Application};
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

#[derive(Clone)]
struct FontAssets {
    bytes: Vec<Vec<u8>>,
    complete: bool,
}

impl FontAssets {
    fn discover() -> Self {
        let assets_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/assets/fonts/JetBrainsMonoNerdFont");
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
    local_applications: Vec<Application>,
    colony_applications: Vec<Application>,
    search_query: String,
    sections: Vec<Section>,
    selected_section: usize,
    status_message: String,
    colony_software: Vec<ColonySoftware>,
    github_status: String,
    font: Font,
}

impl App {
    fn boot(font_assets: FontAssets) -> (Self, Task<Message>) {
        let local_applications = scan::scan_applications().unwrap_or_else(|e| {
            eprintln!("Scan error: {e}");
            Vec::new()
        });

        let status_message = format!("{} applications found", local_applications.len());

        let sections = sections::load_sections();

        let font = font_assets.default_font();

        let app = Self {
            local_applications,
            colony_applications: Vec::new(),
            search_query: String::new(),
            sections,
            selected_section: 0,
            status_message,
            colony_software: Vec::new(),
            github_status: "Scan GitHub en cours...".to_string(),
            font,
        };

        (
            app,
            Task::batch([
                font_assets.load_task(),
                Task::perform(
                    async {
                        github::scan_mothersphere_colony_software()
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::GithubScanFinished,
                ),
                Task::perform(
                    async {
                        colony_apps::scan_colony_applications()
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::ColonyAppsScanFinished,
                ),
            ]),
        )
    }
}

#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    SectionSelected(usize),
    Rescan,
    LaunchApp(Application),
    ClearStatus,
    FontLoaded(Result<(), font::Error>),
    GithubScanFinished(Result<Vec<ColonySoftware>, String>),
    ColonyAppsScanFinished(Result<Vec<Application>, String>),
    ColonyLaunchFinished(Result<(), String>),
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
                }
                Task::none()
            }
            Message::Rescan => {
                match scan::scan_applications() {
                    Ok(apps) => {
                        self.local_applications = apps;
                        self.status_message =
                            format!("{} applications found", self.total_applications());
                    }
                    Err(e) => {
                        self.status_message = format!("Error: {e}");
                    }
                }
                self.github_status = "Scan GitHub en cours...".to_string();
                Task::batch([
                    Task::perform(
                        async {
                            github::scan_mothersphere_colony_software()
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Message::GithubScanFinished,
                    ),
                    Task::perform(
                        async {
                            colony_apps::scan_colony_applications()
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Message::ColonyAppsScanFinished,
                    ),
                ])
            }
            Message::LaunchApp(app) => {
                if app.origin == AppOrigin::Colony {
                    self.status_message = format!("Mise à jour en cours pour {}...", app.name);
                    return Task::perform(
                        async move {
                            colony_apps::update_and_launch(app)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Message::ColonyLaunchFinished,
                    );
                }

                let exec = app.exec.clone();
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
                                        .map_err(|error| format!("Impossible de lancer: {error}"))
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
            Message::ClearStatus => {
                self.status_message = format!("{} applications found", self.total_applications());
                Task::none()
            }
            Message::FontLoaded(_) => Task::none(),
            Message::GithubScanFinished(result) => {
                match result {
                    Ok(items) => {
                        self.github_status =
                            format!("{} logiciel(s) Colony détecté(s) sur GitHub", items.len());
                        self.colony_software = items;
                    }
                    Err(error) => {
                        self.github_status = format!("Erreur GitHub: {error}");
                    }
                }
                Task::none()
            }
            Message::ColonyAppsScanFinished(result) => {
                match result {
                    Ok(apps) => {
                        self.colony_applications = apps;
                        self.status_message =
                            format!("{} applications found", self.total_applications());
                    }
                    Err(error) => {
                        self.status_message = format!("Erreur Colony: {error}");
                    }
                }
                Task::none()
            }
            Message::ColonyLaunchFinished(result) => {
                match result {
                    Ok(()) => {
                        self.status_message = "Application lancée.".to_string();
                        return Task::perform(
                            async {
                                std::thread::sleep(Duration::from_secs(4));
                            },
                            |_| Message::ClearStatus,
                        );
                    }
                    Err(error) => {
                        self.status_message = format!("Erreur Colony: {error}");
                    }
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = self.view_content();

        let main_layout = row![sidebar, content].spacing(0);

        container(main_layout).width(Fill).height(Fill).into()
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
        let search = text_input("Search applications...", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(12)
            .size(16)
            .width(Fill);

        let status = text(&self.status_message)
            .size(12)
            .font(self.app_font())
            .color(color!(0x888899));

        let header = row![search, status]
            .spacing(16)
            .align_y(iced::Alignment::Center);

        let app_grid = self.view_app_grid();
        let github_section = self.view_github_section();

        let content = column![
            header,
            github_section,
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

    fn view_github_section(&self) -> Element<'_, Message> {
        let heading = text("GitHub MotherSphere")
            .size(13)
            .font(self.app_font())
            .color(color!(0x8a8aa3));

        let status = text(&self.github_status)
            .size(12)
            .font(self.app_font())
            .color(color!(0x888899));

        let list: Element<'_, Message> = if self.colony_software.is_empty() {
            text("Aucun repo Colony détecté pour le moment.")
                .size(12)
                .font(self.app_font())
                .color(color!(0x666677))
                .into()
        } else {
            let entries: Vec<Element<'_, Message>> = self
                .colony_software
                .iter()
                .map(|software| {
                    let name = text(&software.name)
                        .size(13)
                        .font(self.app_font())
                        .color(color!(0xffffff));
                    let description = text(
                        software
                            .description
                            .as_deref()
                            .unwrap_or("Aucune description."),
                    )
                    .size(11)
                    .font(self.app_font())
                    .color(color!(0x888899));

                    column![name, description].spacing(2).into()
                })
                .collect();

            Column::with_children(entries).spacing(6).into()
        };

        column![heading, status, list].spacing(6).into()
    }

    fn view_app_card(&self, app: &Application) -> Element<'_, Message> {
        let icon_char = app
            .name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .next()
            .unwrap_or('?');

        let icon = text(icon_char.to_string())
            .size(32)
            .font(self.app_font_with_weight(Weight::Medium))
            .color(color!(0x8888ff));

        let name = text(app.name.clone())
            .size(14)
            .font(self.app_font())
            .color(color!(0xffffff));

        let card_content = column![
            container(icon).width(Fill).center_x(Fill),
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
            .on_press(Message::LaunchApp(app.clone()))
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

    fn filtered_applications(&self) -> Vec<&Application> {
        let query = self.search_query.to_lowercase();
        let selected_section = self.sections.get(self.selected_section);
        self.local_applications
            .iter()
            .chain(self.colony_applications.iter())
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

    fn total_applications(&self) -> usize {
        self.local_applications.len() + self.colony_applications.len()
    }

    fn app_font(&self) -> Font {
        self.font
    }

    fn app_font_with_weight(&self, weight: Weight) -> Font {
        Font {
            weight,
            ..self.font
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

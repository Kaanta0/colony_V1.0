mod config;
mod scan;

use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Column, Row,
};
use iced::{color, Element, Fill, Font, Length, Theme};

use config::Config;
use scan::Repository;

pub fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .window_size((1000.0, 700.0))
        .run()
}

#[derive(Debug, Clone)]
enum Category {
    All,
    Favorites,
    Recent,
    Dev,
    Work,
}

impl Category {
    fn label(&self) -> &'static str {
        match self {
            Category::All => "All",
            Category::Favorites => "Favorites",
            Category::Recent => "Recent",
            Category::Dev => "Dev",
            Category::Work => "Work",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Category::All => "*",
            Category::Favorites => "#",
            Category::Recent => "@",
            Category::Dev => "/",
            Category::Work => "~",
        }
    }
}

impl Default for Category {
    fn default() -> Self {
        Category::All
    }
}

struct App {
    config: Config,
    repositories: Vec<Repository>,
    search_query: String,
    selected_category: Category,
    status_message: String,
}

impl Default for App {
    fn default() -> Self {
        let config = Config::load().unwrap_or_else(|e| {
            eprintln!("Config error: {e}");
            Config {
                scan: config::ScanConfig {
                    directories: vec![],
                    max_depth: 2,
                    interval_seconds: 0,
                },
            }
        });

        let repositories = scan::scan_repositories(&config).unwrap_or_else(|e| {
            eprintln!("Scan error: {e}");
            Vec::new()
        });

        let status_message = format!("{} repositories found", repositories.len());

        Self {
            config,
            repositories,
            search_query: String::new(),
            selected_category: Category::All,
            status_message,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    CategorySelected(Category),
    Rescan,
    OpenRepository(String),
}

impl App {
    fn title(&self) -> String {
        String::from("Colony Launcher")
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::CategorySelected(category) => {
                self.selected_category = category;
            }
            Message::Rescan => {
                match scan::scan_repositories(&self.config) {
                    Ok(repos) => {
                        self.status_message = format!("{} repositories found", repos.len());
                        self.repositories = repos;
                    }
                    Err(e) => {
                        self.status_message = format!("Error: {e}");
                    }
                }
            }
            Message::OpenRepository(path) => {
                self.status_message = format!("Opening: {path}");
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg(&path).spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("explorer").arg(&path).spawn();
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
            .size(28)
            .font(Font::MONOSPACE);

        let categories = [
            Category::All,
            Category::Favorites,
            Category::Recent,
            Category::Dev,
            Category::Work,
        ];

        let category_buttons: Vec<Element<Message>> = categories
            .into_iter()
            .map(|cat| self.view_category_button(cat))
            .collect();

        let category_list = Column::with_children(category_buttons).spacing(4);

        let rescan_btn = button(text("Rescan").size(14))
            .on_press(Message::Rescan)
            .padding([8, 16])
            .width(Fill);

        let sidebar_content = column![
            title,
            container(text("")).height(24),
            category_list,
            container(text("")).height(Length::Fill),
            rescan_btn,
        ]
        .spacing(8)
        .padding(16)
        .width(180);

        container(sidebar_content)
            .style(|_theme| container::Style {
                background: Some(color!(0x1a1a2e).into()),
                ..Default::default()
            })
            .height(Fill)
            .into()
    }

    fn view_category_button(&self, category: Category) -> Element<'_, Message> {
        let is_selected = matches!(
            (&self.selected_category, &category),
            (Category::All, Category::All)
                | (Category::Favorites, Category::Favorites)
                | (Category::Recent, Category::Recent)
                | (Category::Dev, Category::Dev)
                | (Category::Work, Category::Work)
        );

        let label = text(format!("{}  {}", category.icon(), category.label())).size(14);

        let btn = button(label)
            .on_press(Message::CategorySelected(category))
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
        let search = text_input("Search projects...", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(12)
            .size(16)
            .width(Fill);

        let status = text(&self.status_message)
            .size(12)
            .color(color!(0x888899));

        let header = row![search, status]
            .spacing(16)
            .align_y(iced::Alignment::Center);

        let repo_grid = self.view_repository_grid();

        let content = column![
            header,
            container(text("")).height(16),
            repo_grid
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

    fn view_repository_grid(&self) -> Element<'_, Message> {
        let filtered: Vec<&Repository> = self.filtered_repositories();

        if filtered.is_empty() {
            return container(
                text("No repositories found")
                    .size(16)
                    .color(color!(0x666677)),
            )
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into();
        }

        let mut rows: Vec<Element<Message>> = Vec::new();

        for chunk in filtered.chunks(3) {
            let mut row_items: Vec<Element<Message>> = Vec::new();

            for repo in chunk {
                row_items.push(Self::view_repo_card(repo));
            }

            while row_items.len() < 3 {
                row_items.push(container(column![]).width(Fill).into());
            }

            rows.push(Row::with_children(row_items).spacing(16).into());
        }

        let grid = Column::with_children(rows).spacing(16);

        scrollable(grid).height(Fill).into()
    }

    fn view_repo_card(repo: &Repository) -> Element<'_, Message> {
        let icon = text("/")
            .size(24)
            .font(Font::MONOSPACE)
            .color(color!(0x6c6c8a));

        let name = text(repo.name.clone()).size(16).color(color!(0xffffff));

        let path = text(repo.display_path.clone())
            .size(12)
            .color(color!(0x666677));

        let card_content = column![
            icon,
            container(text("")).height(8),
            name,
            path,
        ]
        .spacing(4)
        .padding(16);

        let repo_path = repo.path.clone();
        button(card_content)
            .on_press(Message::OpenRepository(repo_path))
            .padding(0)
            .width(Fill)
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered => color!(0x2a2a4e),
                    button::Status::Pressed => color!(0x3a3a5e),
                    _ => color!(0x1a1a2e),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: color!(0xffffff),
                    border: iced::Border::default().rounded(8),
                    ..Default::default()
                }
            })
            .into()
    }

    fn filtered_repositories(&self) -> Vec<&Repository> {
        let query = self.search_query.to_lowercase();
        self.repositories
            .iter()
            .filter(|repo| {
                if query.is_empty() {
                    return true;
                }
                repo.name.to_lowercase().contains(&query)
                    || repo.display_path.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

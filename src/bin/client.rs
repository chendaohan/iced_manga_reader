use std::{collections::VecDeque, sync::Arc};

use iced::{
    executor,
    font::Family,
    widget::{self, image},
    window, Application, Command, Font, Length, Renderer, Settings, Theme,
};
use iced_aw::helpers;
use protos::manga::{manga_client::MangaClient, ImageNumber, MangaInfo};
use tokio::sync::Mutex;
use tonic::{transport::Channel, Request};

pub mod protos;

const BUFFER_LENGTH: usize = 3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    MangaReader::run(Settings {
        default_font: Font {
            family: Family::Name("Noto Sans CJK HK"),
            ..Default::default()
        },
        window: window::Settings {
            min_size: Some((1300, 768)),
            ..Default::default()
        },
        default_text_size: 22.0,
        ..Default::default()
    })?;
    Ok(())
}

// 滚动的方向
#[derive(Debug, Clone)]
enum ForeAndAft {
    Fore,
    Middle, // 初始化
    Aft,
}

// 标识页面
#[derive(Debug, Clone)]
enum Page {
    Info,
    Image,
}

// 状态修改指令
#[derive(Debug, Clone)]
enum Message {
    GetClient(Arc<Mutex<MangaClient<Channel>>>),
    GetInfo(MangaInfo),
    GetImage {
        current_number: usize,
        fore_and_aft: ForeAndAft,
        image: Vec<u8>,
    },
    ChangePage(Page),
    ChangeImage(usize),
}

// 软件状态
struct MangaReader {
    current_page: Page,
    client: Option<Arc<Mutex<MangaClient<Channel>>>>,
    info: Option<MangaInfo>,
    image_buffer: VecDeque<Vec<u8>>,
    image_height: u64,
    current_image_number: usize,
    current_number: usize,
}

impl Application for MangaReader {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        let reader = Self {
            current_page: Page::Info,
            client: None,
            info: None,
            image_buffer: VecDeque::with_capacity(BUFFER_LENGTH),
            image_height: 1764,
            current_image_number: 0,
            current_number: 0,
        };
        let command = Command::perform(
            async move {
                let client = MangaClient::connect("http://[::1]:8080").await.unwrap();
                Arc::new(Mutex::new(client))
            },
            Message::GetClient,
        );
        (reader, command)
    }

    fn title(&self) -> String {
        String::from("Manga Reader")
    }

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::GetClient(client) => {
                self.client = Some(client);
                let mut batch = Vec::new();
                let client_clone = Arc::clone(self.client.as_ref().unwrap());
                batch.push(Command::perform(
                    async move {
                        let mut client = client_clone.lock().await;
                        client
                            .get_manga_info(Request::new(protos::manga::Empty {}))
                            .await
                            .unwrap()
                            .into_inner()
                    },
                    Message::GetInfo,
                ));
                for index in 0..BUFFER_LENGTH {
                    let client_clone = Arc::clone(self.client.as_ref().unwrap());
                    batch.push(get_manga_image(
                        client_clone,
                        0,
                        index as u32,
                        ForeAndAft::Middle,
                    ));
                }
                Command::batch(batch)
            }
            Message::GetInfo(info) => {
                self.info = Some(info);
                Command::none()
            }
            Message::GetImage {
                current_number,
                fore_and_aft,
                image,
            } => {
                match fore_and_aft {
                    ForeAndAft::Fore => {
                        self.image_buffer.pop_back();
                        self.image_buffer.push_front(image);
                    }
                    ForeAndAft::Middle => {
                        self.image_buffer.push_back(image);
                    }
                    ForeAndAft::Aft => {
                        self.image_buffer.pop_front();
                        self.image_buffer.push_back(image);
                    }
                }
                self.current_image_number = current_number;
                Command::none()
            }
            Message::ChangePage(page) => {
                self.current_page = page;
                Command::none()
            }
            Message::ChangeImage(number) => {
                if number == self.current_number {
                    return Command::none();
                }
                self.current_number = number;
                let pages = self.info.as_ref().unwrap().pages as usize;
                let client = Arc::clone(self.client.as_ref().unwrap());
                if number > self.current_image_number && (2..(pages - 1)).contains(&number) {
                    get_manga_image(
                        client,
                        number,
                        (number + BUFFER_LENGTH / 2) as u32,
                        ForeAndAft::Aft,
                    )
                } else if number < self.current_image_number && (1..(pages - 2)).contains(&number) {
                    get_manga_image(
                        client,
                        number,
                        (number - BUFFER_LENGTH / 2) as u32,
                        ForeAndAft::Fore,
                    )
                } else {
                    Command::none()
                }
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        if let Page::Info = self.current_page {
            info_page(self)
        } else {
            image_page(self)
        }
        .into()
    }
}

fn get_manga_image(
    client: Arc<Mutex<MangaClient<Channel>>>,
    current_number: usize,
    number: u32,
    fore_and_aft: ForeAndAft,
) -> Command<Message> {
    Command::perform(
        async move {
            let mut client = client.lock().await;
            client
                .get_manga_image(Request::new(ImageNumber { number }))
                .await
                .unwrap()
                .into_inner()
                .image
        },
        move |image| Message::GetImage {
            current_number,
            fore_and_aft,
            image,
        },
    )
}

fn info_page(state: &MangaReader) -> widget::Container<Message, Renderer> {
    if let Some(manga) = &state.info {
        widget::container(widget::row!(
            widget::image(image::Handle::from_memory(manga.cover.clone())),
            widget::column!(
                {
                    let mut grid = iced_aw::Grid::with_columns(2);
                    grid.insert(widget::text("id:"));
                    grid.insert(widget::text(manga.id));
                    grid.insert(widget::text("english name:"));
                    grid.insert(widget::text(&manga.english_name));
                    grid.insert(widget::text("japanese name:"));
                    grid.insert(widget::text(&manga.japanese_name));
                    grid.insert(widget::text("tags:"));
                    grid.insert(helpers::wrap_horizontal(
                        manga
                            .tags
                            .iter()
                            .map(|tag| iced_aw::badge(widget::text(tag)).into())
                            .collect(),
                    ));
                    grid.insert(widget::text("artists:"));
                    grid.insert(helpers::wrap_horizontal(
                        manga
                            .artists
                            .iter()
                            .map(|artist| iced_aw::badge(widget::text(artist)).into())
                            .collect(),
                    ));
                    grid.insert(widget::text("pages:"));
                    grid.insert(widget::text(manga.pages));
                    grid.insert(widget::text("uploaded:"));
                    grid.insert(widget::text(&manga.uploaded));
                    grid
                },
                widget::button(widget::text("Read Manga"))
                    .on_press(Message::ChangePage(Page::Image))
            )
        ))
        .center_x()
        .center_y()
        .width(Length::Fill)
        .height(Length::Fill)
    } else {
        widget::container(widget::Space::new(Length::Shrink, Length::Shrink))
    }
}

fn image_page(state: &MangaReader) -> widget::Container<Message, Renderer> {
    widget::container(widget::column!(
        widget::button(widget::text("Back Info")).on_press(Message::ChangePage(Page::Info)),
        widget::scrollable(widget::column({
            println!(
                "current number = {}, buffer length = {}",
                state.current_image_number,
                state.image_buffer.len()
            );
            if state.image_buffer.len() < BUFFER_LENGTH {
                vec![widget::Space::new(Length::Shrink, Length::Shrink).into()]
            } else {
                let pages = state.info.as_ref().unwrap().pages as usize;
                let mut list = Vec::with_capacity(pages);
                match state.current_image_number {
                    number if (0..(BUFFER_LENGTH / 2)).contains(&number) => {
                        for index in 0..BUFFER_LENGTH {
                            list.push(
                                make_image(&state.image_buffer, index, state.image_height).into(),
                            );
                        }
                        for _ in BUFFER_LENGTH..pages {
                            list.push(make_space(state.image_height).into());
                        }
                    }
                    number if ((pages - (BUFFER_LENGTH / 2))..pages).contains(&number) => {
                        for _ in 0..(pages - BUFFER_LENGTH) {
                            list.push(make_space(state.image_height).into());
                        }
                        for index in 0..BUFFER_LENGTH {
                            list.push(
                                make_image(&state.image_buffer, index, state.image_height).into(),
                            );
                        }
                    }
                    number => {
                        for _ in 0..(number - (BUFFER_LENGTH / 2)) {
                            list.push(make_space(state.image_height).into());
                        }
                        for index in 0..BUFFER_LENGTH {
                            list.push(
                                make_image(&state.image_buffer, index, state.image_height).into(),
                            );
                        }
                        for _ in (number + (BUFFER_LENGTH / 2 + 1))..pages {
                            list.push(make_space(state.image_height).into());
                        }
                    }
                }
                list
            }
        }))
        .on_scroll(|viewport| {
            //println!("{viewport:?}");
            let y = viewport.absolute_offset().y as usize;
            let current_number = if y == 0 {
                0
            } else {
                y / state.image_height as usize
            };
            Message::ChangeImage(current_number)
        })
    ))
    .center_x()
    .width(Length::Fill)
}

fn make_space(height: u64) -> widget::Space {
    widget::Space::new(Length::Shrink, Length::Fixed(height as f32))
}

fn make_image(
    buffer: &VecDeque<Vec<u8>>,
    index: usize,
    height: u64,
) -> widget::Image<image::Handle> {
    widget::image(image::Handle::from_memory(
        buffer.get(index).unwrap().clone(),
    ))
    .height(Length::Fixed(height as f32))
}

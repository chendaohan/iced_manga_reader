use std::{
    fs::{self, File},
    path::PathBuf,
};

use protos::manga::{manga_server::Manga, Empty, Image, ImageNumber, MangaInfo};
use serde::{Deserialize, Serialize};
use tokio::signal;
use tonic::{transport::Server, Request, Response, Status};

use crate::protos::manga::manga_server::MangaServer;

pub mod protos;

const ADDRESS: &str = "[::1]:8080";

#[derive(Debug, Serialize, Deserialize)]
struct MangaJson {
    id: u32,
    english_name: String,
    japanese_name: String,
    tags: Vec<String>,
    artists: Vec<String>,
    pages: u32,
    uploaded: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = ADDRESS.parse()?;
    let server = Server::builder()
        .add_service(MangaServer::new(MangaService))
        .serve(address);
    let ctrl_c = signal::ctrl_c();

    tokio::select! {
        result = server => println!("{result:?}"),
        result = ctrl_c => println!("{result:?}"),
    }

    Ok(())
}

struct MangaService;

#[tonic::async_trait]
impl Manga for MangaService {
    async fn get_manga_info(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<MangaInfo>, Status> {
        let manga_path =
            File::open("assets/manga.json").map_err(|err| Status::not_found(err.to_string()))?;
        let manga_json: MangaJson =
            serde_json::from_reader(manga_path).map_err(|err| Status::unknown(err.to_string()))?;
        let manga_cover =
            fs::read("assets/cover.jpg").map_err(|err| Status::not_found(err.to_string()))?;
        let manga_info = MangaInfo {
            id: manga_json.id,
            english_name: manga_json.english_name,
            japanese_name: manga_json.japanese_name,
            cover: manga_cover,
            tags: manga_json.tags,
            artists: manga_json.artists,
            pages: manga_json.pages,
            uploaded: manga_json.uploaded,
        };
        Ok(Response::new(manga_info))
    }

    async fn get_manga_image(
        &self,
        request: Request<ImageNumber>,
    ) -> Result<Response<Image>, Status> {
        let image_number = request.into_inner().number;
        println!("image number = {image_number}");
        let image_path = PathBuf::from(format!("assets/images/{image_number}.jpg"));
        let image = fs::read(image_path)?;
        Ok(Response::new(Image { image }))
    }
}

use crate::config::Config;
use async_trait::async_trait;
use reqwest_middleware::ClientBuilder;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde_json::Value;
use std::error::Error;
use std::time::Duration;

use super::{Twitch, Youtube};

#[allow(dead_code)]
/// Status of the live stream
pub enum Status {
    /// Stream is online.
    Online,
    /// Stream is offline.
    Offline,
    /// The status of the stream could not be determined.
    Unknown,
}

#[async_trait]
pub trait Live {
    async fn get_status(&self) -> Result<bool, Box<dyn Error>>;
    fn room(&self) -> &str;
    async fn get_real_m3u8_url(&self) -> Result<String, Box<dyn Error>>;
}
pub async fn select_live(cfg: Config) -> Result<Box<dyn Live>, Box<dyn Error>> {
    // 设置最大重试次数为4294967295次
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(4294967295);
    let raw_client = reqwest::Client::builder()
        .cookie_store(true)
        // 设置超时时间为30秒
        .timeout(Duration::new(30, 0))
        .build()
        .unwrap();
    let client = ClientBuilder::new(raw_client.clone())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    match cfg.platform.as_str() {
        "Youtube" => Ok(Box::new(Youtube::new(
            cfg.youtube.room.as_str(),
            cfg.youtube.access_token,
            client.clone(),
        ))),
        "Twitch" => Ok(Box::new(Twitch::new(
            cfg.twitch.room.as_str(),
            client.clone(),
        ))),
        "YoutubePreviewLive" => {
            let room_id = get_live_id(cfg.youtube_preview_live.channel_id.as_str())
                .await
                .unwrap();
            Ok(Box::new(Youtube::new(
                room_id.as_str(),
                cfg.youtube.access_token,
                client.clone(),
            )))
        }
        _ => Err("unknown platform".into()),
    }
}

// https://www.youtube.com/channel/UC1zFJrfEKvCixhsjNSb1toQ
// 通过channel_name获取channel_id
async fn get_channel_id(channel_name: &str) -> Result<String, Box<dyn Error>> {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(4294967295);
    let raw_client = reqwest::Client::builder()
        .cookie_store(true)
        // 设置超时时间为30秒
        .timeout(Duration::new(30, 0))
        .build()
        .unwrap();
    let client = ClientBuilder::new(raw_client.clone())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();
    let url = format!("https://www.youtube.com/c/{}", channel_name);
    let res = client.get(&url).send().await?;
    let body = res.text().await?;
    let room_id = body
        .split("\"channelId\":\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .nth(0)
        .unwrap();
    Ok(room_id.to_string())
}

// 通过channel_id获取live_id
pub async fn get_live_id(channel_name: &str) -> Result<String, Box<dyn Error>> {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(1);
    let raw_client = reqwest::Client::builder()
        .cookie_store(true)
        // 设置超时时间为30秒
        .timeout(Duration::new(30, 0))
        .build()
        .unwrap();
    let client = ClientBuilder::new(raw_client.clone())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();
    let url = format!("https://www.youtube.com/channel/{}", channel_name);
    tracing::debug!("{}", url);
    let res = client.get(&url).send().await?;
    let body = res.text().await?;
    // 保存body为文件,后缀为html
    let html = prettyish_html::prettify(body.as_str());
    // let mut file = std::fs::File::create("body.html").unwrap();
    // std::io::Write::write_all(&mut file, html.as_bytes()).unwrap();

    let re = regex::Regex::new(r#"\s*<script nonce=".*">var ytInitialData = (.*);\s*?</script>"#)
        .unwrap();
    // if re.is_match(html.as_str()) {
    //     let live_id = re.captures(html.as_str()).unwrap().get(1).unwrap().as_str();
    //     let live_id = live_id.split("\"").nth(1).unwrap();
    //     println!("{}", live_id);
    // } else {
    //     println!("no match");
    // }
    for cap in re.captures(html.as_str()) {
        let json = cap.get(1).unwrap().as_str();
        // let json = json.split(";").nth(0).unwrap();
        // let json = json.split("=").nth(1).unwrap();
        // let json = json.split(";").nth(0).unwrap();
        // let json = json.split("}").nth(0).unwrap();
        // let json = json.split("{").nth(1).unwrap();
        // let json = json.split("\"").nth(1).unwrap();
        // let json = json.split("\"").nth(0).unwrap();
        let j: Value = serde_json::from_str(json).unwrap();
        let video_id = j["contents"]["twoColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]
            ["content"]["sectionListRenderer"]["contents"][1]["itemSectionRenderer"]["contents"][0]
            ["shelfRenderer"]["content"]["horizontalListRenderer"]["items"][0]["gridVideoRenderer"]
            ["videoId"]
            .to_string();

        tracing::debug!(
            "{}",
            j["contents"]["twoColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]
                ["sectionListRenderer"]["contents"][1]["itemSectionRenderer"]["contents"][0]
                ["shelfRenderer"]["content"]["horizontalListRenderer"]["items"][0]
                ["gridVideoRenderer"]["videoId"]
        );
        // 将结果保存为一个json文件
        // let mut file = std::fs::File::create("live_id.json").unwrap();
        // std::io::Write::write_all(&mut file, json.as_bytes()).unwrap();
        return Ok(video_id);
    }

    Err("获取video_id失败".into())
}

// 测试get_room_id 传入UC1zFJrfEKvCixhsjNSb1toQ
#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }
    #[test]
    fn test_get_room_id() {
        let channel_id = "GameSpun";
        let r = aw!(get_channel_id(channel_id)).unwrap();
        println!("id:{}", r);
    }
    #[test]
    fn test_get_live_id() {
        let channel_id = "UC1zFJrfEKvCixhsjNSb1toQ";
        let r = aw!(get_live_id(channel_id)).unwrap();
        println!("id:{}", r);
    }
}

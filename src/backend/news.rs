use crate::types::NewsItem;
use anyhow::Result;

pub async fn fetch_arch_news() -> Result<Vec<NewsItem>> {
    let content = reqwest::get("https://archlinux.org/feeds/news/")
        .await?
        .bytes()
        .await?;

    let channel = rss::Channel::read_from(&content[..])?;

    let items: Vec<NewsItem> = channel
        .items()
        .iter()
        .take(15)
        .map(|item| {
            NewsItem {
                title: item.title().unwrap_or("(no title)").to_string(),
                link: item.link().unwrap_or("").to_string(),
                description: item
                    .description()
                    .unwrap_or("")
                    // Strip HTML tags for TUI display
                    .replace("<p>", "")
                    .replace("</p>", "\n")
                    .replace("<a href=", "")
                    .replace("</a>", "")
                    .replace("<code>", "`")
                    .replace("</code>", "`")
                    .replace("<em>", "")
                    .replace("</em>", "")
                    .replace("<strong>", "")
                    .replace("</strong>", "")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&")
                    .chars()
                    .filter(|c| *c != '>' && *c != '"')
                    .collect::<String>()
                    .trim()
                    .to_string(),
                pub_date: item.pub_date().unwrap_or("unknown").to_string(),
            }
        })
        .collect();

    Ok(items)
}

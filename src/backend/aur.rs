use serde::Deserialize;
use crate::types::Package;
use anyhow::Result;

const AUR_RPC_URL: &str = "https://aur.archlinux.org/rpc/";

#[derive(Deserialize)]
struct RpcResponse {
    version: i32,
    #[serde(rename = "type")]
    type_: String,
    resultcount: usize,
    results: Vec<RpcPackage>,
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct RpcPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Maintainer")]
    maintainer: Option<String>,
    #[serde(rename = "URL")]
    url: Option<String>,
    #[serde(rename = "NumVotes")]
    num_votes: u32,
    #[serde(rename = "Popularity")]
    popularity: f64,
    #[serde(rename = "LastModified")]
    last_modified: u64,
    #[serde(rename = "OutOfDate")]
    out_of_date: Option<u64>,
}

pub struct AurClient {
    client: reqwest::Client,
}

impl AurClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let url = format!("{}?v=5&type=search&arg={}", AUR_RPC_URL, query);
        let resp: RpcResponse = self.client.get(&url).send().await?.json().await?;
        
        if resp.type_ == "error" {
            return Err(anyhow::anyhow!("AUR RPC Error: {:?}", resp.error));
        }

        Ok(resp.results.into_iter().map(|p| p.into_package()).collect())
    }

    pub async fn info(&self, packages: &[&str]) -> Result<Vec<Package>> {
        if packages.is_empty() {
             return Ok(Vec::new());
        }

        // AUR RPC supports multiple ?arg[]=...
        let mut url = format!("{}?v=5&type=info", AUR_RPC_URL);
        for p in packages {
            url.push_str(&format!("&arg[]={}", p));
        }

        let resp: RpcResponse = self.client.get(&url).send().await?.json().await?;

        Ok(resp.results.into_iter().map(|p| p.into_package()).collect())
    }
}

impl RpcPackage {
    fn into_package(self) -> Package {
        Package {
            name: self.name,
            version: self.version,
            description: self.description,
            maintainer: self.maintainer,
            url: self.url,
            votes: self.num_votes,
            popularity: self.popularity,
            last_modified: self.last_modified,
            out_of_date: self.out_of_date,
            installed_version: None, // RPC doesn't know local state
            repository: "aur".to_string(),
        }
    }
}

use std::env;
use std::fmt;
use std::error::Error;
use std::fs::{self, File};
use std::path::Path;
use std::io::Write;

const URL: &str = "https://www.gotransit.com/en/trip-planning/seeschedules/full-schedules";
const TEMP_SUBDIR_NAME: &str = "sched";

#[derive(Debug, Clone)]
struct ParseError;
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unable to parse HTML document")
    }
}
impl Error for ParseError {}

#[derive(Clone)]
struct ScheduleNotFoundError { name: String }
impl fmt::Display for ScheduleNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Schedule not found: {}", self.name)
    }
}
impl fmt::Debug for ScheduleNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Schedule not found: {}", self.name)
    }
}
impl Error for ScheduleNotFoundError {}

struct TempFile { filename: Box<Path> }
impl TempFile {
    fn get(name: &str) -> Self {
        let mut pathbuf = env::temp_dir();
        pathbuf.push(TEMP_SUBDIR_NAME);
        fs::create_dir_all(&pathbuf).unwrap_or_default();

        pathbuf.push(name);
        TempFile { filename: pathbuf.into_boxed_path() }
    }
    fn create(&self) -> Result<File, std::io::Error> {
        File::create(&self.filename)
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        fs::remove_file(&self.filename).unwrap_or_default();
    }
}

fn get_normalized_name(name: &str) -> String {
    let lower_name = name.to_ascii_lowercase();
    match lower_name.as_ref() {
        // full names
        "lakeshore west" => "01-18",
        "milton" => "21",
        "kitchener" => "30-31-33",
        "barrie" => "63-65-68",
        "richmond hill" => "61",
        "stouffville" => "70-71",
        "lakeshore east" => "09-90",
        // short names
        "lw" => "01-18",
        "mi" => "21",
        "ki" => "30-31-33",
        "ba" => "63-65-68",
        "rh" => "61",
        "st" => "70-71",
        "le" => "09-90",
        // combos
        "1" => "01-18",
        "01" => "01-18",
        "18" => "01-18",
        "30" => "30-31-33",
        "31" => "30-31-33",
        "33" => "30-31-33",
        "63" => "63-65-68",
        "65" => "63-65-68",
        "68" => "63-65-68",
        "70" => "70-71",
        "71" => "70-71",
        "9" => "09-90",
        "09" => "09-90",
        "90" => "09-90",
        "41" => "41-45-47-48",
        "45" => "41-45-47-48",
        "47" => "41-45-47-48",
        "48" => "41-45-47-48",
        "52" => "52-54-56",
        "54" => "52-54-56",
        "56" => "52-54-56",
        // else
        other => other,
    }.to_string()
}

async fn download_full_schedules_page() -> Result<String, Box<dyn Error>> {
    let resp = reqwest::get(URL).await?;
    Ok(resp.text().await?)
}

async fn find_pdf_link(name: &str) -> Result<String, Box<dyn Error>> {
    let raw_html = download_full_schedules_page().await?;
    let document = scraper::Html::parse_document(&raw_html);
    let tbody_selector = scraper::Selector::parse("table[class='content-page-table']>tbody")?;
    let tbody = document.select(&tbody_selector).next().ok_or(ParseError)?;

    let tr_selector = scraper::Selector::parse("tr")?;
    let key_selector = scraper::Selector::parse("strong")?;
    let link_selector = scraper::Selector::parse("a")?;

    for tr in tbody.select(&tr_selector) {
        let key = tr.select(&key_selector).next().ok_or(ParseError)?;
        let link = tr.select(&link_selector).next().ok_or(ParseError)?;

        if key.inner_html().to_ascii_lowercase() == name
            || link.inner_html().to_ascii_lowercase() == name {
            return Ok(link.value().attr("href")
                .ok_or(ParseError)?
                .to_string());
        }
    }

    Err(Box::new(ScheduleNotFoundError { name: name.to_string() }))
}

async fn download_pdf(
    url: reqwest::Url,
    temp_file: &TempFile,
) -> Result<(), Box<dyn Error>> {
    let response = reqwest::get(url).await?;
    let mut file = temp_file.create()?;
    let content = response.bytes().await?;
    file.write_all(&content)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: sched <name>");
        return Ok(());
    }

    let name = args[1..].join(" ");
    let name = get_normalized_name(&name);
    println!("Getting schedule for {}", name);

    let url = match find_pdf_link(&name).await {
        Ok(href) => {
            let base_url = reqwest::Url::parse(&URL).unwrap();
            base_url.join(&href).unwrap()
        }
        Err(e) => return Err(e)
    };
    println!("PDF link: {}", url);

    let temp_file = TempFile::get("sched.pdf");
    println!("Saving to {}", temp_file.filename.display());
    download_pdf(url, &temp_file).await?;

    open::that(temp_file.filename.as_os_str())?;

    std::thread::sleep(std::time::Duration::new(2, 0));
    Ok(())
}

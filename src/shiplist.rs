use scraper::{Html, Selector};
use regex::Regex;
use log::{info};

use crate::download::download;

fn get_country_ships(country: &str) -> Vec<String> {
    let url = format!("https://gamemodels3d.com/games/worldofwarships/vehicles/{}", country);
    let page = download(&url);

    let document = Html::parse_document(&page);
    let a_selector = Selector::parse("a").unwrap();
    let mut ids = vec!();
    for element in document.select(&a_selector) {
        let href = element.value().attr("href");
        let re = Regex::new(r"/games/worldofwarships/vehicles/(\w+\d+)").unwrap();
        if let Some(href) = href {
            for capture in re.captures_iter(href) {
                ids.push(capture[1].to_string());
            }
        }
    }
    info!("Found {} ships for country {}", ids.len(), country);
    ids
}

pub fn get_ship_list() -> Vec<String> {
    let countries = [
        "japan",
        "usa",
        "germany",
        "ussr",
        "uk",
        "panasia",
        "france",
        "commonwealth",
        "italy",
        "pan_america",
        "europe",
    ];

    let mut ships = vec!();
    for country in countries.iter() {
        info!("Loading ships for country {}...", country);
        ships.append(&mut get_country_ships(country));
    }
    info!("Found {} ships", ships.len());
    ships
}

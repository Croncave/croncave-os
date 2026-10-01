//! Demo sites the Watcher's types read in the prototype: a listings site that gains
//! listings on demand, and a page whose text can be changed. They are ordinary web pages
//! that computers fetch over HTTP.

use std::sync::Mutex;

use axum::extract::State;
use axum::response::Html;

use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct Listing {
    pub id: u32,
    pub title: String,
    pub area: String,
    pub price: u32,
    pub beds: u32,
}

pub struct DemoState {
    pub listings: Mutex<Vec<Listing>>,
    pub page: Mutex<(u32, String)>,
}

const AREAS: &[&str] = &["Mission", "Sunset", "Noe Valley", "Richmond", "SoMa", "Bernal Heights", "Dogpatch"];
const KINDS: &[&str] =
    &["Sunny flat", "Garden studio", "Top-floor apartment", "Quiet one-bedroom", "Bright loft", "Victorian unit"];

impl Default for DemoState {
    fn default() -> Self {
        let seed = [
            (1, "Sunny flat near the park", "Sunset", 2650, 2),
            (2, "Garden studio", "Mission", 1850, 0),
            (3, "Top-floor apartment with a view", "Noe Valley", 3400, 2),
            (4, "Quiet one-bedroom", "Richmond", 2150, 1),
            (5, "Bright loft", "SoMa", 2950, 1),
        ];
        Self {
            listings: Mutex::new(
                seed.into_iter()
                    .map(|(id, t, a, p, b)| Listing { id, title: t.into(), area: a.into(), price: p, beds: b })
                    .collect(),
            ),
            page: Mutex::new((1, "Open Monday to Saturday, 9am to 6pm.".into())),
        }
    }
}

impl DemoState {
    /// Plant a new listing (Dev tools). Every few, one is cheap enough to match a typical watch.
    pub fn add_listing(&self) -> Listing {
        let mut l = self.listings.lock().expect("listings");
        let id = l.iter().map(|x| x.id).max().unwrap_or(0) + 1;
        let n = id as usize;
        let listing = Listing {
            id,
            title: format!("{} #{id}", KINDS[n % KINDS.len()]),
            area: AREAS[n % AREAS.len()].into(),
            price: 1600 + ((n * 373) % 1800) as u32,
            beds: (n % 3) as u32 + 1,
        };
        l.insert(0, listing.clone());
        listing
    }

    pub fn change_page(&self) -> String {
        let mut p = self.page.lock().expect("page");
        p.0 += 1;
        p.1 = match p.0 % 3 {
            0 => "Open Monday to Saturday, 9am to 6pm.".into(),
            1 => "Open every day, 8am to 8pm. Now open on Sundays!".into(),
            _ => "Closed for renovation until next month.".into(),
        };
        p.1.clone()
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub async fn listings(State(app): State<AppState>) -> Html<String> {
    let l = app.demo.listings.lock().expect("listings").clone();
    let items: String = l
        .iter()
        .map(|x| {
            format!(
                r#"<li class="listing"><a class="title" data-id="L{id}" href="/demo/listings/{id}">{t}</a> <span class="area">{a}</span> · <span class="price">${p} / mo</span> · <span class="beds">{b} bd</span></li>"#,
                id = x.id,
                t = esc(&x.title),
                a = esc(&x.area),
                p = x.price,
                b = x.beds
            )
        })
        .collect();
    Html(format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Demo Rentals</title></head><body><h1>Demo Rentals</h1><p>A local stand-in for a listings site. Dev tools can add a listing.</p><ul>{items}</ul></body></html>"
    ))
}

pub async fn page(State(app): State<AppState>) -> Html<String> {
    let (v, text) = app.demo.page.lock().expect("page").clone();
    Html(format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Corner Bakery</title></head><body><h1>Corner Bakery</h1><p id=\"hours\">{}</p><p>Fresh bread every morning.</p><footer>Page version {v}</footer></body></html>",
        esc(&text)
    ))
}

use crate::Settings;

use mostro_core::order::{Kind as OrderKind, SmallOrder as Order, Status};
use nostr_sdk::prelude::*;
use std::str::FromStr;
use uuid::Uuid;

pub fn order_from_tags(event: Event) -> Result<Order> {
    let tags = event.tags;
    let mut order = Order {
        created_at: Some(event.created_at.as_u64() as i64),
        ..Default::default()
    };
    for tag in tags {
        let t = tag.as_slice();
        let v = t.get(1).unwrap().as_str();
        match t.first().unwrap().as_str() {
            "d" => {
                let id = v.parse::<Uuid>();
                let id = match id {
                    core::result::Result::Ok(id) => id,
                    Err(_) => continue,
                };
                order.id = Some(id);
            }
            "k" => {
                order.kind = Some(OrderKind::from_str(v).unwrap());
            }
            "f" => {
                order.fiat_code = v.to_string();
            }
            "s" => {
                order.status = Some(Status::from_str(v).unwrap_or(Status::Dispute));
            }
            "amt" => {
                order.amount = v.parse::<i64>().unwrap();
            }
            "fa" => {
                if v.contains('.') {
                    continue;
                }
                let max = t.get(2);
                if max.is_some() {
                    order.min_amount = v.parse::<i64>().ok();
                    order.max_amount = max.unwrap().parse::<i64>().ok();
                } else {
                    let fa = v.parse::<i64>();
                    order.fiat_amount = fa.unwrap_or(0);
                }
            }
            "pm" => {
                order.payment_method = v.to_string();
            }
            "premium" => {
                order.premium = v.parse::<i64>().unwrap();
            }
            _ => {}
        }
    }

    Ok(order)
}

pub async fn connect_nostr() -> Result<Client> {
    let my_keys = Keys::generate();

    let relays = Settings::get().relays.clone();
    // Create new client
    let client = Client::new(my_keys);
    // Add relays
    for r in relays.into_iter() {
        client.add_relay(r).await?;
    }
    // Connect to relays and keep connection alive
    client.connect().await;

    Ok(client)
}

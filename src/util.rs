use crate::my_order::Order;
use mostro_core::order::{Kind as OrderKind, Status};
use nostr_sdk::prelude::*;
use std::str::FromStr;

pub fn order_from_tags(event: Event) -> Result<Order> {
    let tags = event.tags;
    let mut order = Order {
        created_at: event.created_at,
        ..Default::default()
    };
    for tag in tags {
        let t = tag.as_slice();
        let v = t.get(1).unwrap().as_str();
        match t.first().unwrap().as_str() {
            "d" => {
                order.id = v.to_string();
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

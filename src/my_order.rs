use mostro_core::order::{Kind as OrderKind, Status};
use nostr_sdk::prelude::*;

#[derive(Debug, Default, Clone)]
pub struct Order {
    pub id: String,
    pub kind: Option<OrderKind>,
    pub fiat_code: String,
    pub status: Option<Status>,
    pub amount: i64,
    pub min_amount: Option<i64>,
    pub max_amount: Option<i64>,
    pub fiat_amount: i64,
    pub payment_method: String,
    pub premium: i64,
    pub created_at: Timestamp,
}

impl Order {
    pub fn sats_amount(&self) -> String {
        if self.amount == 0 {
            "Market price".to_string()
        } else {
            self.amount.to_string()
        }
    }

    pub fn fiat_amount(&self) -> String {
        if self.max_amount.is_some() {
            format!("{}-{}", self.min_amount.unwrap(), self.max_amount.unwrap())
        } else {
            self.fiat_amount.to_string()
        }
    }
}

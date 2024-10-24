use crate::settings::get_settings_path;
use sqlx::pool::Pool;
use sqlx::Sqlite;
use sqlx::SqlitePool;
use std::fs::File;
use std::path::Path;

pub async fn connect() -> Result<Pool<Sqlite>, sqlx::Error> {
    let mostrui_dir = get_settings_path();
    let mostrui_db_path = format!("{}/mostrui.db", mostrui_dir);

    if !Path::exists(Path::new(&mostrui_db_path)) {
        if let Err(res) = File::create(&mostrui_db_path) {
            println!("Error in creating db file: {}", res)
        }
    }

    let db_url = format!("sqlite://{}", mostrui_db_path);
    let pool = SqlitePool::connect(&db_url).await?;

    // We create the database file with orders table if the file doesn't exists
    if !Path::new(&mostrui_db_path).exists() {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS orders (
                id TEXT PRIMARY KEY,
                kind TEXT,
                status TEXT,
                amount INTEGER NOT NULL,
                fiat_code TEXT NOT NULL,
                min_amount INTEGER,
                max_amount INTEGER,
                fiat_amount INTEGER NOT NULL,
                payment_method TEXT NOT NULL,
                premium INTEGER NOT NULL,
                master_buyer_pubkey TEXT,
                master_seller_pubkey TEXT,
                buyer_invoice TEXT,
                created_at INTEGER,
                expires_at INTEGER,
                buyer_token INTEGER,
                seller_token INTEGER
            );
            "#,
        )
        .execute(&pool)
        .await?;
    }

    Ok(pool)
}

#[derive(Debug, Default, Clone)]
pub struct Order {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub amount: i64,
    pub fiat_code: String,
    pub min_amount: Option<i64>,
    pub max_amount: Option<i64>,
    pub fiat_amount: i64,
    pub payment_method: String,
    pub premium: i64,
    pub master_buyer_pubkey: Option<String>,
    pub master_seller_pubkey: Option<String>,
    pub buyer_invoice: Option<String>,
    pub created_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub buyer_token: Option<u16>,
    pub seller_token: Option<u16>,
}

impl Order {
    // Setters encadenables
    pub fn set_kind(&mut self, kind: String) -> &mut Self {
        self.kind = Some(kind);
        self
    }

    pub fn set_status(&mut self, status: String) -> &mut Self {
        self.status = Some(status);
        self
    }

    pub fn set_amount(&mut self, amount: i64) -> &mut Self {
        self.amount = amount;
        self
    }

    pub fn set_fiat_code(&mut self, fiat_code: String) -> &mut Self {
        self.fiat_code = fiat_code;
        self
    }

    pub fn set_min_amount(&mut self, min_amount: i64) -> &mut Self {
        self.min_amount = Some(min_amount);
        self
    }

    pub fn set_max_amount(&mut self, max_amount: i64) -> &mut Self {
        self.max_amount = Some(max_amount);
        self
    }

    pub fn set_fiat_amount(&mut self, fiat_amount: i64) -> &mut Self {
        self.fiat_amount = fiat_amount;
        self
    }

    pub fn set_payment_method(&mut self, payment_method: String) -> &mut Self {
        self.payment_method = payment_method;
        self
    }

    pub fn set_premium(&mut self, premium: i64) -> &mut Self {
        self.premium = premium;
        self
    }

    // Applying changes to the database
    pub async fn save(&self, pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
        // Validation if an identity document is present
        if let Some(ref id) = self.id {
            sqlx::query(
                r#"
              UPDATE orders 
              SET kind = ?, status = ?, amount = ?, fiat_code = ?, min_amount = ?, max_amount = ?, 
                  fiat_amount = ?, payment_method = ?, premium = ?, master_buyer_pubkey = ?, 
                  master_seller_pubkey = ?, buyer_invoice = ?, created_at = ?, expires_at = ?, 
                  buyer_token = ?, seller_token = ?
              WHERE id = ?
              "#,
            )
            .bind(&self.kind)
            .bind(&self.status)
            .bind(self.amount)
            .bind(&self.fiat_code)
            .bind(self.min_amount)
            .bind(self.max_amount)
            .bind(self.fiat_amount)
            .bind(&self.payment_method)
            .bind(self.premium)
            .bind(&self.master_buyer_pubkey)
            .bind(&self.master_seller_pubkey)
            .bind(&self.buyer_invoice)
            .bind(self.created_at)
            .bind(self.expires_at)
            .bind(self.buyer_token)
            .bind(self.seller_token)
            .bind(id)
            .execute(pool)
            .await?;

            println!("Order with id {} updated in the database.", id);
        } else {
            println!("Order must have an ID to be updated.");
        }

        Ok(())
    }
}

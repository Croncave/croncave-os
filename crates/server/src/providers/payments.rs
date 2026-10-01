//! Payments. Stripe holds subscriptions and cards in production; our ledger works out
//! usage and sends one charge a month. The `mock` provider accepts Stripe's documented
//! test card numbers and behaves like they do.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// What the checkout form collects. With real Stripe this becomes a payment method id
/// from Stripe's own card form, so card numbers never reach our servers.
#[derive(Debug, Clone, Deserialize)]
pub struct CardInput {
    pub number: String,
    pub exp_month: u32,
    pub exp_year: u32,
    pub cvc: String,
    #[serde(default)]
    pub zip: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Card {
    pub brand: String,
    pub last4: String,
    /// The same card always has the same fingerprint (awards are tied to it).
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Charge {
    pub id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    /// A message to show the person.
    #[error("{0}")]
    Declined(String),
    #[error("payments are not configured: {0}")]
    NotConfigured(String),
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    async fn create_customer(&self, account: Uuid, email: &str) -> Result<String, PaymentError>;
    async fn attach_card(&self, customer: &str, card: &CardInput) -> Result<Card, PaymentError>;
    async fn charge(&self, customer: &str, amount_micros: i64, description: &str) -> Result<Charge, PaymentError>;
}

pub struct MockPayments {
    /// customer -> the test card on file
    cards: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl Default for MockPayments {
    fn default() -> Self {
        Self { cards: std::sync::Mutex::new(Default::default()) }
    }
}

fn luhn(number: &str) -> bool {
    let digits: Vec<u32> = number.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 12 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| {
            if i % 2 == 1 {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            } else {
                *d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn brand(number: &str) -> &'static str {
    match number.chars().next() {
        Some('4') => "Visa",
        Some('5') => "Mastercard",
        Some('3') => "American Express",
        Some('6') => "Discover",
        _ => "Card",
    }
}

#[async_trait]
impl PaymentProvider for MockPayments {
    async fn create_customer(&self, account: Uuid, _email: &str) -> Result<String, PaymentError> {
        Ok(format!("cus_mock_{}", account.simple()))
    }

    async fn attach_card(&self, customer: &str, card: &CardInput) -> Result<Card, PaymentError> {
        let number: String = card.number.chars().filter(|c| c.is_ascii_digit()).collect();
        if !luhn(&number) {
            return Err(PaymentError::Declined("That card number isn't valid.".into()));
        }
        let now = chrono::Utc::now();
        let (y, m) = (chrono::Datelike::year(&now) as u32, chrono::Datelike::month(&now));
        let year = if card.exp_year < 100 { 2000 + card.exp_year } else { card.exp_year };
        if card.exp_month == 0 || card.exp_month > 12 || (year, card.exp_month) < (y, m) {
            return Err(PaymentError::Declined("Your card has expired.".into()));
        }
        if card.cvc.len() < 3 {
            return Err(PaymentError::Declined("Check the security code.".into()));
        }
        // Stripe's documented test numbers.
        match number.as_str() {
            "4000000000000002" => return Err(PaymentError::Declined("Your card was declined.".into())),
            "4000000000000069" => return Err(PaymentError::Declined("Your card has expired.".into())),
            "4000000000000127" => return Err(PaymentError::Declined("Your card's security code is incorrect.".into())),
            _ => {}
        }
        self.cards.lock().expect("cards").insert(customer.to_string(), number.clone());
        Ok(Card {
            brand: brand(&number).into(),
            last4: number[number.len() - 4..].to_string(),
            fingerprint: hex::encode(Sha256::digest(format!("card:{number}").as_bytes()))[..24].to_string(),
        })
    }

    async fn charge(&self, customer: &str, amount_micros: i64, _description: &str) -> Result<Charge, PaymentError> {
        let card = self.cards.lock().expect("cards").get(customer).cloned();
        // After a restart the mock has no memory of cards; treat the customer as good.
        if card.as_deref() == Some("4000000000009995") && amount_micros > 0 {
            return Err(PaymentError::Declined("Your card has insufficient funds.".into()));
        }
        Ok(Charge { id: format!("ch_mock_{}", Uuid::new_v4().simple()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(n: &str) -> CardInput {
        CardInput { number: n.into(), exp_month: 12, exp_year: 2099, cvc: "123".into(), zip: "10001".into() }
    }

    #[tokio::test]
    async fn stripe_test_cards_behave_like_stripe() {
        let p = MockPayments::default();
        let ok = p.attach_card("c1", &card("4242 4242 4242 4242")).await.unwrap();
        assert_eq!((ok.brand.as_str(), ok.last4.as_str()), ("Visa", "4242"));
        assert!(p.charge("c1", 10_000_000, "Plus").await.is_ok());

        assert!(matches!(p.attach_card("c2", &card("4000000000000002")).await, Err(PaymentError::Declined(_))));
        assert!(matches!(p.attach_card("c2", &card("4242424242424241")).await, Err(PaymentError::Declined(_))));

        p.attach_card("c3", &card("4000000000009995")).await.unwrap();
        assert!(p.charge("c3", 1, "x").await.is_err(), "insufficient funds fails at charge time");
    }

    #[tokio::test]
    async fn same_card_same_fingerprint() {
        let p = MockPayments::default();
        let a = p.attach_card("a", &card("4242424242424242")).await.unwrap();
        let b = p.attach_card("b", &card("4242-4242-4242-4242")).await.unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
    }
}

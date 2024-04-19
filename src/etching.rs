use super::*;
use crate::rune::Rune;

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Copy, Clone, Eq)]
pub struct Terms {
  pub amount: Option<u128>,
  pub cap: Option<u128>,
  pub height: (Option<u64>, Option<u64>),
  pub offset: (Option<u64>, Option<u64>),
}

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Copy, Clone, Eq)]
pub struct Etching {
  pub divisibility: Option<u8>,
  pub premine: Option<u128>,
  pub rune: Option<Rune>,
  pub spacers: Option<u32>,
  pub symbol: Option<char>,
  pub terms: Option<Terms>,
  pub turbo: bool,
}

impl Etching {
  pub const MAX_DIVISIBILITY: u8 = 38;
  pub const MAX_SPACERS: u32 = 0b00000111_11111111_11111111_11111111;

  pub fn supply(&self) -> Option<u128> {
    let premine = self.premine.unwrap_or_default();
    let cap = self.terms.and_then(|terms| terms.cap).unwrap_or_default();
    let amount = self
      .terms
      .and_then(|terms| terms.amount)
      .unwrap_or_default();
    premine.checked_add(cap.checked_mul(amount)?)
  }
}

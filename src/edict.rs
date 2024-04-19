use super::*;
use crate::rune_id::RuneId;

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Copy, Clone, Eq)]
pub struct Edict {
  pub id: RuneId,
  pub amount: u128,
  pub output: u32,
}
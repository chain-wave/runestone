use super::*;
use crate::flaw::Flaw;
use std::collections::HashMap;
use std::collections::VecDeque;

pub(super) struct Message {
  pub(super) flaw: Option<Flaw>,
  pub(super) edicts: Vec<Edict>,
  pub(super) fields: HashMap<u128, VecDeque<u128>>,
}
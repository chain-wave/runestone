use {super::*, flag::Flag, tag::Tag};

use crate::flaw::Flaw;
use bitcoin_arch::constants::MAX_SCRIPT_ELEMENT_SIZE;

mod flag;
mod tag;

use bitcoin_arch::script;
use bitcoin_arch::script::builder;
use serde::{Serialize, Deserialize};
use crate::edict::*;
use crate::etching::*;
use crate::rune_id::*;
use crate::varint::*;

use bitcoin_arch::opcodes;
use bitcoin_arch::script::ScriptBuf;
use bitcoin_arch::script::builder::ScriptBuilder;

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Runestone {
    pub edicts: Vec<Edict>,
    pub etching: Option<Etching>,
    pub mint: Option<RuneId>,
    pub pointer: Option<u32>,
}

#[derive(Debug, PartialEq)]
enum Payload {
    Valid(Vec<u8>),
    Invalid(Flaw),
}

impl Runestone {
    pub const MAGIC_NUMBER: opcodes::Opcode = opcodes::all::OP_PUSHNUM_13;
    pub const COMMIT_CONFIRMATIONS: u16 = 6;

    pub fn encipher(&self) -> ScriptBuf {
        let mut payload = Vec::new();

        if let Some(etching) = self.etching {
        let mut flags = 0;
        Flag::Etching.set(&mut flags);

        if etching.terms.is_some() {
            Flag::Terms.set(&mut flags);
        }

        if etching.turbo {
            Flag::Turbo.set(&mut flags);
        }

        Tag::Flags.encode([flags], &mut payload);

        Tag::Rune.encode_option(etching.rune.map(|rune| rune.0), &mut payload);
        Tag::Divisibility.encode_option(etching.divisibility, &mut payload);
        Tag::Spacers.encode_option(etching.spacers, &mut payload);
        Tag::Symbol.encode_option(etching.symbol, &mut payload);
        Tag::Premine.encode_option(etching.premine, &mut payload);

        if let Some(terms) = etching.terms {
            Tag::Amount.encode_option(terms.amount, &mut payload);
            Tag::Cap.encode_option(terms.cap, &mut payload);
            Tag::HeightStart.encode_option(terms.height.0, &mut payload);
            Tag::HeightEnd.encode_option(terms.height.1, &mut payload);
            Tag::OffsetStart.encode_option(terms.offset.0, &mut payload);
            Tag::OffsetEnd.encode_option(terms.offset.1, &mut payload);
        }
        }

        if let Some(RuneId { block, tx }) = self.mint {
        Tag::Mint.encode([block.into(), tx.into()], &mut payload);
        }

        Tag::Pointer.encode_option(self.pointer, &mut payload);

        if !self.edicts.is_empty() {
        varint::encode_to_vec(Tag::Body.into(), &mut payload);

        let mut edicts = self.edicts.clone();
        edicts.sort_by_key(|edict| edict.id);

        let mut previous = RuneId::default();
        for edict in edicts {
            let (block, tx) = previous.delta(edict.id).unwrap();
            varint::encode_to_vec(block, &mut payload);
            varint::encode_to_vec(tx, &mut payload);
            varint::encode_to_vec(edict.amount, &mut payload);
            varint::encode_to_vec(edict.output.into(), &mut payload);
            previous = edict.id;
        }
        }

        let mut builder = ScriptBuilder::new();
        builder.push_opcode(opcodes::all::OP_RETURN);
        builder.push_opcode(Runestone::MAGIC_NUMBER);

        for chunk in payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
            builder.push_slice_only(chunk);
        }

        builder.into_script()
    }

    fn integers(payload: &[u8]) -> Result<Vec<u128>, varint::Error> {
        let mut integers = Vec::new();
        let mut i = 0;

        while i < payload.len() {
        let (integer, length) = varint::decode(&payload[i..])?;
        integers.push(integer);
        i += length;
        }

        Ok(integers)
    }
}

use {super::*, flag::Flag, tag::Tag};

use crate::artifact::Artifact;
use crate::cenotaph::Cenotaph;
use crate::flaw::Flaw;
use crate::rune::Rune;
use crate::runestone::message::Message;
use bitcoin_arch::constants::MAX_SCRIPT_ELEMENT_SIZE;

mod flag;
mod message;
mod tag;

use crate::edict::*;
use crate::etching::*;
use crate::rune_id::*;
use bitcoin_arch::script::builder;
use serde::{Deserialize, Serialize};

use bitcoin_arch::opcodes;
use bitcoin_arch::script::builder::ScriptBuilder;
use bitcoin_arch::script::{
    instructions::{Instruction, Instructions},
    ScriptBuf,
};
use bitcoin_arch::transaction::Transaction;

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

    pub fn decipher(transaction: &Transaction) -> Option<Artifact> {
        let payload = match Runestone::payload(transaction) {
            Some(Payload::Valid(payload)) => payload,
            Some(Payload::Invalid(flaw)) => {
                return Some(Artifact::Cenotaph(Cenotaph {
                    flaw: Some(flaw),
                    ..Default::default()
                }));
            }
            None => return None,
        };

        let Ok(integers) = Runestone::integers(&payload) else {
            return Some(Artifact::Cenotaph(Cenotaph {
                flaw: Some(Flaw::Varint),
                ..Default::default()
            }));
        };

        let Message {
            mut flaw,
            edicts,
            mut fields,
        } = Message::from_integers(transaction, &integers);

        let mut flags = Tag::Flags
            .take(&mut fields, |[flags]| Some(flags))
            .unwrap_or_default();

        let etching = Flag::Etching.take(&mut flags).then(|| Etching {
            divisibility: Tag::Divisibility.take(&mut fields, |[divisibility]| {
                let divisibility = u8::try_from(divisibility).ok()?;
                (divisibility <= Etching::MAX_DIVISIBILITY).then_some(divisibility)
            }),
            premine: Tag::Premine.take(&mut fields, |[premine]| Some(premine)),
            rune: Tag::Rune.take(&mut fields, |[rune]| Some(Rune(rune))),
            spacers: Tag::Spacers.take(&mut fields, |[spacers]| {
                let spacers = u32::try_from(spacers).ok()?;
                (spacers <= Etching::MAX_SPACERS).then_some(spacers)
            }),
            symbol: Tag::Symbol.take(&mut fields, |[symbol]| {
                char::from_u32(u32::try_from(symbol).ok()?)
            }),
            terms: Flag::Terms.take(&mut flags).then(|| Terms {
                cap: Tag::Cap.take(&mut fields, |[cap]| Some(cap)),
                height: (
                    Tag::HeightStart.take(&mut fields, |[start_height]| {
                        u64::try_from(start_height).ok()
                    }),
                    Tag::HeightEnd.take(&mut fields, |[start_height]| {
                        u64::try_from(start_height).ok()
                    }),
                ),
                amount: Tag::Amount.take(&mut fields, |[amount]| Some(amount)),
                offset: (
                    Tag::OffsetStart.take(&mut fields, |[start_offset]| {
                        u64::try_from(start_offset).ok()
                    }),
                    Tag::OffsetEnd.take(&mut fields, |[end_offset]| u64::try_from(end_offset).ok()),
                ),
            }),
            turbo: Flag::Turbo.take(&mut flags),
        });

        let mint = Tag::Mint.take(&mut fields, |[block, tx]| {
            RuneId::new(block.try_into().ok()?, tx.try_into().ok()?)
        });

        let pointer = Tag::Pointer.take(&mut fields, |[pointer]| {
            let pointer = u32::try_from(pointer).ok()?;
            (u64::from(pointer) < u64::try_from(transaction.output.len()).unwrap())
                .then_some(pointer)
        });

        if etching
            .map(|etching| etching.supply().is_none())
            .unwrap_or_default()
        {
            flaw.get_or_insert(Flaw::SupplyOverflow);
        }

        if flags != 0 {
            flaw.get_or_insert(Flaw::UnrecognizedFlag);
        }

        if fields.keys().any(|tag| tag % 2 == 0) {
            flaw.get_or_insert(Flaw::UnrecognizedEvenTag);
        }

        if let Some(flaw) = flaw {
            return Some(Artifact::Cenotaph(Cenotaph {
                flaw: Some(flaw),
                mint,
                etching: etching.and_then(|etching| etching.rune),
            }));
        }

        Some(Artifact::Runestone(Runestone {
            edicts,
            etching,
            mint,
            pointer,
        }))
    }

    fn payload(transaction: &Transaction) -> Option<Payload> {
        // search transaction outputs for payload
        for output in &transaction.output {
            let mut instructions = Instructions::from(output.script_pubkey.as_slice());

            // payload starts with OP_RETURN
            if instructions.next() != Some(Ok(Instruction::Op(opcodes::all::OP_RETURN))) {
                continue;
            }

            // followed by the protocol identifier, ignoring errors, since OP_RETURN
            // scripts may be invalid
            if instructions.next() != Some(Ok(Instruction::Op(Runestone::MAGIC_NUMBER))) {
                continue;
            }

            // construct the payload by concatenating remaining data pushes
            let mut payload = Vec::new();

            for result in instructions {
                match result {
                    Ok(Instruction::PushBytes(push)) => {
                        payload.extend_from_slice(push);
                    }
                    Ok(Instruction::Op(_)) => {
                        return Some(Payload::Invalid(Flaw::Opcode));
                    }
                    Err(_) => {
                        return Some(Payload::Invalid(Flaw::InvalidScript));
                    }
                }
            }

            return Some(Payload::Valid(payload));
        }

        None
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

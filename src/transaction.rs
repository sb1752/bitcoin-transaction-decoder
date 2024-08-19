use serde::{ser::{SerializeSeq, SerializeStruct}, Serialize, Serializer};
use std::io::{BufRead, Write};
use std::fmt;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    UnsupportedSegwitFlag(u8),
    ParseFailed(&'static str)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref e) => write!(f, "IO error: {}", e),
            Error::UnsupportedSegwitFlag(swflag) => write!(f, "unsupported segwit version: {}", swflag),
            Error::ParseFailed(s) => write!(f, "parse failed: {}", s)
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub struct Transaction {
    pub version: u32,
    pub inputs: Vec<TxIn>, 
    pub outputs: Vec<TxOut>,
    pub lock_time: u32    
}

impl Transaction {
    pub fn compute_txid(&self) -> Txid {
        let mut txid_data = Vec::new();
        self.version.consensus_encode(&mut txid_data).expect("writing to a vec shouldn't fail");
        self.inputs.consensus_encode(&mut txid_data).expect("writing to a vec shouldn't fail");
        self.outputs.consensus_encode(&mut txid_data).expect("writing to a vec shouldn't fail");
        self.lock_time.consensus_encode(&mut txid_data).expect("writing to a vec shouldn't fail");
        Txid::from_raw_transaction(txid_data)
    }
}

impl Serialize for Transaction {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tx = serializer.serialize_struct("Transaction", 5)?;
        tx.serialize_field("transaction_id", &self.compute_txid())?;
        tx.serialize_field("version", &self.version)?;
        tx.serialize_field("inputs", &self.inputs)?;
        tx.serialize_field("outputs", &self.outputs)?;
        tx.serialize_field("lock_time", &self.lock_time)?;
        tx.end()
    }
}

#[derive(Debug)]
pub struct Txid([u8; 32]);

impl Txid {
    pub fn from_hash(bytes: [u8; 32]) -> Txid {
        Txid(bytes)
    }

    fn from_raw_transaction(tx: Vec<u8>) -> Txid {
        let mut hasher = Sha256::new();
        hasher.update(&tx);
        let hash1 = hasher.finalize();
    
        let mut hasher = Sha256::new();
        hasher.update(hash1);
        let hash2 = hasher.finalize();
    
        Txid::from_hash(hash2.into())        
    }
}

impl Serialize for Txid {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
       let mut bytes = self.0.clone();
       bytes.reverse();
       s.serialize_str(&hex::encode(bytes))
    }
}

#[derive(Debug)]
pub struct TxIn {
    pub txid: Txid,
    pub output_index: u32,
    pub script_sig: String,
    pub witness: Witness,
    pub sequence: u32
}

impl Serialize for TxIn {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer {
        let mut txin = s.serialize_struct("TxIn", 5)?;
        txin.serialize_field("txid", &self.txid)?;
        txin.serialize_field("vout", &self.output_index)?;
        txin.serialize_field("script_sig", &self.script_sig)?;
        
        if !&self.witness.is_empty() {
            txin.serialize_field("txinwitness", &self.witness)?;
        }
        
        txin.serialize_field("sequence", &self.sequence)?;
        txin.end()
    }
}

#[derive(Debug)]
pub struct Witness {
    content: Vec<Vec<u8>>
}

impl Witness {
    pub fn new() -> Self {
        Witness { content: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

impl Serialize for Witness {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer {
        let mut seq = s.serialize_seq(Some(self.content.len()))?;

        for elem in self.content.iter() {
            seq.serialize_element(&hex::encode(&elem))?;
        }

        seq.end()
    }
}

#[derive(Debug)]
pub struct Amount(u64);

impl Amount {
    pub fn from_sat(satoshi: u64) -> Amount {
        Amount(satoshi)
    }
}

trait BitcoinValue {
    fn to_btc(&self) -> f64;
}

impl BitcoinValue for Amount {
    fn to_btc(&self) -> f64 {
        self.0 as f64 / 100_000_000.0
    }
}

#[derive(Debug, Serialize)]
pub struct TxOut {
    #[serde(serialize_with = "as_btc")]
    pub amount: Amount,
    pub script_pubkey: String,
}

fn as_btc<S: Serializer, T: BitcoinValue>(t: &T, s: S) -> Result<S::Ok, S::Error> {
    let btc = t.to_btc();
    s.serialize_f64(btc)
}

#[derive(Debug, Serialize)]
pub struct CompactSize(pub u64);

pub trait Decodable: Sized {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error>;
}

impl Decodable for u8 {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let mut buffer = [0; 1];
        r.read_exact(&mut buffer).map_err(Error::Io)?;
        Ok(buffer[0])
    }
}

impl Decodable for u16 {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let mut buffer = [0; 2];
        r.read_exact(&mut buffer).map_err(Error::Io)?;
        Ok(u16::from_le_bytes(buffer))
    }
}

impl Decodable for u32 {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let mut buffer = [0; 4];
        r.read_exact(&mut buffer).map_err(Error::Io)?;
        Ok(u32::from_le_bytes(buffer))
    }
}

impl Decodable for u64 {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let mut buffer = [0; 8];
        r.read_exact(&mut buffer).map_err(Error::Io)?;
        Ok(u64::from_le_bytes(buffer))
    }
}

impl Decodable for CompactSize {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let n = u8::consensus_decode(r)?;

        match n {
            0xFF => {
                let x = u64::consensus_decode(r)?;
                Ok(CompactSize(x))
            },
            0xFE => {
                let x = u32::consensus_decode(r)?;
                Ok(CompactSize(x as u64))
            },
            0xFD => {
                let x = u16::consensus_decode(r)?;
                Ok(CompactSize(x as u64))
            }
            n => Ok(CompactSize(n as u64))
        }
    }
}

impl Decodable for String {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let len = CompactSize::consensus_decode(r)?.0;
        let mut buffer = vec![0; len as usize];
        r.read_exact(&mut buffer).map_err(Error::Io)?;
        Ok(hex::encode(buffer))
    }    
}

impl Decodable for Vec<TxIn> {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let count = CompactSize::consensus_decode(r)?.0;
        let mut inputs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            inputs.push(TxIn::consensus_decode(r)?);
        }
        Ok(inputs)
    }
}

impl Decodable for TxIn {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        Ok(TxIn {
            txid: Txid::consensus_decode(r)?,
            output_index: u32::consensus_decode(r)?,
            script_sig: String::consensus_decode(r)?,
            sequence: u32::consensus_decode(r)?,
            witness: Witness::new(),
        })
    }
}

impl Decodable for Witness {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let mut witness_items = vec![];
        let count = u8::consensus_decode(r)?;
        for _ in 0..count {
            let len = CompactSize::consensus_decode(r)?.0;
            let mut buffer = vec![0; len as usize];
            r.read_exact(&mut buffer).map_err(Error::Io)?;
            witness_items.push(buffer);
        }
        Ok(Witness {content: witness_items})
    }
}

impl Decodable for Txid {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let mut buffer = [0; 32];
        r.read_exact(&mut buffer).map_err(Error::Io)?;
        Ok(Txid(buffer))
    }    
}

impl Decodable for Vec<TxOut> {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let count = CompactSize::consensus_decode(r)?.0;
        let mut outputs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            outputs.push(TxOut::consensus_decode(r)?);
        }
        Ok(outputs)
    }
}

impl Decodable for TxOut {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        Ok(TxOut {
            amount: Amount::from_sat(u64::consensus_decode(r)?),
            script_pubkey: String::consensus_decode(r)?
        })
    }    
}

impl Decodable for Transaction {
    fn consensus_decode<R: BufRead + ?Sized>(r: &mut R) -> Result<Self, Error> {
        let version = u32::consensus_decode(r)?;
        let inputs = Vec::<TxIn>::consensus_decode(r)?;
        if inputs.is_empty() {
            let segwit_flag = u8::consensus_decode(r)?;
            match segwit_flag {
                1 => {
                    let mut inputs = Vec::<TxIn>::consensus_decode(r)?;
                    let outputs = Vec::<TxOut>::consensus_decode(r)?;
                    for txin in inputs.iter_mut() {
                        txin.witness = Witness::consensus_decode(r)?;
                    }
                    if !inputs.is_empty() && inputs.iter().all(|input| input.witness.is_empty()) {
                        Err(Error::ParseFailed("witness flag set but no witnesses present"))
                    } else {
                        Ok(Transaction {
                            version,
                            inputs,
                            outputs,
                            lock_time: u32::consensus_decode(r)?,
                        })
                    }
                },
                // We don't support anything else
                x => Err(Error::UnsupportedSegwitFlag(x))
            }
        } else {
            Ok(Transaction {
                version,
                inputs,
                outputs: Vec::<TxOut>::consensus_decode(r)?,
                lock_time: u32::consensus_decode(r)?
            })
        }
    }    
}

pub trait Encodable {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error>;
}

impl Encodable for u8 {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let len = w.write([*self].as_slice()).map_err(Error::Io)?;
        Ok(len)
    }
}

impl Encodable for u16 {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let b = self.to_le_bytes();
        let len = w.write(b.as_slice()).map_err(Error::Io)?;
        Ok(len)
    }
}

impl Encodable for u32 {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let b = self.to_le_bytes();
        let len = w.write(b.as_slice()).map_err(Error::Io)?;
        Ok(len)
    }
}

impl Encodable for u64 {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let b = self.to_le_bytes();
        let len = w.write(b.as_slice()).map_err(Error::Io)?;
        Ok(len)
    }
}

impl Encodable for [u8; 32] {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let len = w.write(self.as_slice()).map_err(Error::Io)?;
        Ok(len)
    }
}

impl Encodable for String {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let b = hex::decode(self).expect("should be a valid hex string");
        let compact_size_len = CompactSize(b.len() as u64).consensus_encode(w)?;
        let b_len = w.write(&b).map_err(Error::Io)?;
        Ok(compact_size_len + b_len)
    }
}

impl Encodable for CompactSize {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        match self.0 {
            0..=0xFC => {
                (self.0 as u8).consensus_encode(w)?;
                Ok(1)
            }
            0xFD..=0xFFFF => {
                w.write([0xFD].as_slice()).map_err(Error::Io)?;
                (self.0 as u16).consensus_encode(w)?;
                Ok(3)
            }
            0x10000..=0xFFFFFFFF => {
                w.write([0xFE].as_slice()).map_err(Error::Io)?;
                (self.0 as u32).consensus_encode(w)?;
                Ok(5)
            }
            _ => {
                w.write([0xFF].as_slice()).map_err(Error::Io)?;
                self.0.consensus_encode(w)?;
                Ok(9)
            }
        }
    }
}

impl Encodable for Vec<TxIn> {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let mut len = 0;
        len += CompactSize(self.len() as u64).consensus_encode(w)?;
        for tx in self.iter() {
            len += tx.consensus_encode(w)?;
        }
        Ok(len)
    }
}

impl Encodable for Txid {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        Ok(self.0.consensus_encode(w)?)
    }
}

impl Encodable for TxIn {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let mut len = 0;
        len += self.txid.consensus_encode(w)?;
        len += self.output_index.consensus_encode(w)?;
        len += self.script_sig.consensus_encode(w)?;
        len += self.sequence.consensus_encode(w)?;
        Ok(len)
    }
}

impl Encodable for Vec<TxOut> {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let mut len = 0;
        len += CompactSize(self.len() as u64).consensus_encode(w)?;
        for tx in self.iter() {
            len += tx.consensus_encode(w)?;
        }
        Ok(len)
    }
}

impl Encodable for Amount {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let len = self.0.consensus_encode(w)?;
        Ok(len)
    }
}

impl Encodable for TxOut {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, Error> {
        let mut len = 0;
        len += self.amount.consensus_encode(w)?;
        len += self.script_pubkey.consensus_encode(w)?;
        Ok(len)
    }
}

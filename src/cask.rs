use std::collections::HashMap;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::{fs::File, path::Path};

use crate::{error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

const MAGIC: &[u8; 4] = b"MONA";
const VERSION: u8 = 1;
const MAX_BIN: usize = 127;
const MAX_KEY_SIZE: usize = u8::MAX as usize;
const MAX_VAL_SIZE: usize = u16::MAX as usize;
const SIZE_OF_HEAD: usize = 13;
const SIZE_OF_RECORD_PREFIX: usize = 4; // |u8|u8|u16|

pub struct Cask {
    bins: LogBins,
    log_r: LogReader,
    log_w: LogWriter,
}

impl Cask {
    pub fn new<P>(path: P) -> Result<Cask>
    where
        P: AsRef<Path>,
    {
        let mut f = File::create(&path)?;
        let mut b = BytesMut::with_capacity(SIZE_OF_HEAD);
        b.put(&MAGIC[..]);
        b.put_u8(VERSION);
        b.put_u64(0u64);
        f.write_all(&b)?;
        Self::open(path)
    }

    pub fn open<P>(path: P) -> Result<Cask>
    where
        P: AsRef<Path>,
    {
        let bins = LogBins::new();
        let log_r = LogReader::open(&path)?;
        let log_w = LogWriter::open(&path)?;
        Cask { bins, log_r, log_w }.restore()
    }

    pub fn count(&self, bin: usize) -> usize {
        self.bins.count(bin)
    }

    pub fn put(&mut self, bin: usize, key: Bytes, val: Bytes) -> Result<()> {
        // put to log
        let rec = LogRecord::new(bin, key.clone(), val)?;
        let ptr = self.log_w.append(rec)?;
        // put to bins
        self.bins.put(bin, key, ptr)
    }

    pub fn get(&mut self, bin: usize, key: Bytes) -> Result<Option<Bytes>> {
        if let Some(ptr) = self.bins.get(bin, key)? {
            // println!("ptr(pos={}, len={})", ptr.pos, ptr.len);
            let record = self.log_r.get(ptr)?;
            // println!("{:?}", record);
            return Ok(Some(record.val));
        }
        Ok(None)
    }

    pub fn del(&mut self, _key: Bytes) -> Result<bool> {
        Ok(false)
    }

    pub fn scan(&mut self, bin: usize) -> Result<Vec<Bytes>> {
        self.log_r.scan(bin)
    }

    pub fn close(self) {
        drop(self.log_r);
        drop(self.log_w);
    }

    fn restore(mut self) -> Result<Self> {
        self.log_r.restore(&mut self.bins)?;
        Ok(self)
    }
}

type LogBin = HashMap<Bytes, LogPtr>;

struct LogBins {
    bins: Vec<Option<LogBin>>,
}

impl LogBins {
    pub fn new() -> Self {
        const NONE: Option<LogBin> = None;
        LogBins {
            bins: Vec::from([NONE; MAX_BIN]),
        }
    }

    pub fn count(&self, bin: usize) -> usize {
        self.bins[bin].as_ref().map_or(0, |m| m.len())
    }

    pub fn put(&mut self, bin: usize, key: Bytes, ptr: LogPtr) -> Result<()> {
        if bin > MAX_BIN {
            error!("bin cannot exceed {}", MAX_BIN)
        }
        if self.bins[bin].is_none() {
            self.bins[bin] = Some(LogBin::new());
        }
        self.bins[bin].as_mut().unwrap().insert(key, ptr);
        Ok(())
    }

    pub fn get(&self, bin: usize, key: Bytes) -> Result<Option<LogPtr>> {
        if bin > MAX_BIN {
            error!("bin cannot exceed {}", MAX_BIN)
        }
        let bin = self.bins[bin].as_ref().unwrap();
        let ptr = bin.get(&key).copied();
        Ok(ptr)
    }
}

struct LogWriter {
    pos: u64,
    log: BufWriter<File>,
}

impl LogWriter {
    pub fn new(mut log: File) -> Result<Self> {
        let pos = log.seek(SeekFrom::End(0))?;
        let log = BufWriter::new(log);
        Ok(LogWriter { pos, log })
    }

    pub fn open<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let f = File::options().write(true).open(&path)?;
        let w = LogWriter::new(f)?;
        Ok(w)
    }

    pub fn append(&mut self, record: LogRecord) -> Result<LogPtr> {
        // create buffer
        let pos = self.pos;
        let buf: Bytes = record.into();
        // append to file
        self.pos += self.log.write(&buf)? as u64;
        // TODO remove me
        self.log.flush()?;
        // done
        Ok(LogPtr {
            pos,
            len: self.pos - pos,
        })
    }
}

struct LogReader {
    log: File,
}

impl LogReader {
    pub fn new(log: File) -> Result<Self> {
        Ok(LogReader { log })
    }

    pub fn open<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut f = File::options().read(true).open(&path)?;
        let mut buf = BytesMut::zeroed(SIZE_OF_HEAD);
        f.read_exact(&mut buf)?;
        let head = buf.freeze();
        assert_eq!(head.slice(0..4), &MAGIC[..]);
        assert_eq!(head.slice(4..5).get_u8(), VERSION);
        LogReader::new(f)
    }

    pub fn get(&mut self, ptr: LogPtr) -> Result<LogRecord> {
        let mut buf = BytesMut::zeroed(ptr.len as usize);
        self.log.read_exact_at(&mut buf, ptr.pos)?;
        Ok(buf.freeze().into())
    }

    pub fn scan(&mut self, bin: usize) -> Result<Vec<Bytes>> {
        let mut values: Vec<Bytes> = vec![];
        // bug .. assumes no body!!
        let mut pos: u64 = SIZE_OF_HEAD as u64;
        let end: u64 = self.log.metadata()?.len();
        self.log.seek(SeekFrom::Start(pos))?;

        while pos < end {
            // read prefix
            let mut buf = BytesMut::zeroed(SIZE_OF_RECORD_PREFIX);
            self.log.read_exact(&mut buf)?;
            let mut prefix = buf.freeze();
            let this_bin = prefix.get_u8() as usize;
            let sizeof_k = prefix.get_u8();
            let sizeof_v = prefix.get_u16();
            if this_bin != bin {
                // skip
                self.log
                    .seek(SeekFrom::Current(sizeof_k as i64 + sizeof_v as i64))?;
            } else {
                // read
                self.log.seek(SeekFrom::Current(sizeof_k as i64))?;
                let mut val = BytesMut::zeroed(sizeof_v as usize);
                self.log.read_exact(&mut val)?;
                values.push(val.freeze());
            }
            let len = (SIZE_OF_RECORD_PREFIX as u64) + (sizeof_k as u64) + (sizeof_v as u64);
            pos += len
        }
        Ok(values)
    }

    pub fn restore(&mut self, bins: &mut LogBins) -> Result<()> {
        // funky factoring, and should be an Iterator<LogHint> so the cask owns restore
        // but this is easy right now!
        // #[derive(Debug, Clone)]
        // struct LogHint {
        //     bin: u8,
        //     key: Bytes,
        //     ptr: LogPtr,
        // }

        // bug .. assumes no body!!
        let mut pos: u64 = SIZE_OF_HEAD as u64;
        let end: u64 = self.log.metadata()?.len();
        self.log.seek(SeekFrom::Start(pos))?;

        while pos < end {
            // read prefix
            let mut buf = BytesMut::zeroed(SIZE_OF_RECORD_PREFIX);
            self.log.read_exact(&mut buf)?;
            let mut prefix = buf.freeze();
            let bin = prefix.get_u8() as usize;
            let sizeof_k = prefix.get_u8();
            let sizeof_v = prefix.get_u16();
            // read key
            let mut key = BytesMut::zeroed(sizeof_k as usize);
            self.log.read_exact(&mut key)?;
            // skip val
            self.log.seek(SeekFrom::Current(sizeof_v as i64))?;
            // bins[bin][key] = ptr
            let len = (SIZE_OF_RECORD_PREFIX as u64) + (sizeof_k as u64) + (sizeof_v as u64);
            let ptr = LogPtr { pos, len };
            bins.put(bin, key.freeze(), ptr)?;
            // next
            pos += len
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct LogPtr {
    pos: u64,
    len: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct LogRecord {
    bin: u8,
    key: Bytes,
    val: Bytes,
}

impl LogRecord {
    pub fn new(bin: usize, key: Bytes, val: Bytes) -> Result<Self> {
        if bin > MAX_BIN {
            error!("bin too large (max {})", MAX_BIN)
        }
        if key.len() > MAX_KEY_SIZE {
            error!("key too large (max {} bytes)", MAX_KEY_SIZE)
        }
        if val.len() > MAX_VAL_SIZE {
            error!("val too large (max {} bytes)", MAX_VAL_SIZE)
        }
        Ok(LogRecord {
            bin: bin as u8,
            key,
            val,
        })
    }
}

impl From<LogRecord> for Bytes {
    fn from(value: LogRecord) -> Self {
        // calculate size (make first bit 0)
        let flag: u8 = 0b0111111u8 & value.bin;
        let k_size: usize = value.key.len();
        let v_size: usize = value.val.len();
        // write to buffer
        let mut buf = BytesMut::with_capacity(4 + k_size + v_size);
        buf.put_u8(flag);
        buf.put_u8(k_size as u8);
        buf.put_u16(v_size as u16);
        buf.put(value.key);
        buf.put(value.val);
        buf.freeze()
    }
}

impl From<Bytes> for LogRecord {
    fn from(value: Bytes) -> Self {
        // read prefix
        let bin = *value.first().unwrap();
        let n = value.slice(1..2).get_u8() as usize;
        let m = value.slice(2..4).get_u16() as usize;
        // read key-value
        let i = 4;
        let j = i + n;
        let k = j + m;
        let key = value.slice(i..j);
        let val = value.slice(j..k);
        LogRecord { bin, key, val }
    }
}

#[cfg(test)]
mod test {
    use crate::{cask::Cask, Result};
    use bytes::{BufMut, Bytes, BytesMut};

    fn cask() -> Result<Cask> {
        let tmp_dir = tempfile::tempdir()?;
        let tmp_pth = tmp_dir.path().join("mona.cask");
        Cask::new(&tmp_pth)
    }

    fn put_many(cask: &mut Cask, bin: usize, n: usize) -> Result<()> {
        for i in 0..n {
            let mut k = BytesMut::new();
            k.put(&b"key-"[..]);
            k.put(i.to_string().as_bytes());
            let mut v = BytesMut::new();
            v.put(&b"val-"[..]);
            v.put(i.to_string().as_bytes());
            cask.put(bin, k.freeze(), v.freeze())?;
        }
        Ok(())
    }

    #[test]
    fn test_put_one() -> Result<()> {
        let mut cask = cask()?;
        let k = b"hello".as_slice();
        let v = b"world".as_slice();
        cask.put(0, k.into(), v.into())?;
        assert_eq!(1, cask.count(0), "bin 0 should have 1 record");
        cask.close();
        Ok(())
    }

    #[test]
    fn test_put_many() -> Result<()> {
        let mut cask = cask()?;
        let n: usize = 11;
        let m: usize = 27;
        put_many(&mut cask, 0, n)?;
        put_many(&mut cask, 1, m)?;
        assert_eq!(n, cask.count(0), "bin 0 should have {} records", n);
        assert_eq!(m, cask.count(1), "bin 1 should have {} records", m);
        Ok(())
    }

    #[test]
    fn test_get_one_of_one() -> Result<()> {
        let mut cask = cask()?;
        // put one
        let k = b"hello".as_slice();
        let v = b"world".as_slice();
        cask.put(0, k.into(), v.into())?;
        // get one
        let res = cask.get(0, k.into())?;
        assert_eq!(Some(v.into()), res, "expected Some(b\"world\")");
        Ok(())
    }

    #[test]
    fn test_get_one_of_many() -> Result<()> {
        let mut cask = cask()?;
        put_many(&mut cask, 0, 100)?;
        assert_eq!(Some(b"val-0"[..].into()), cask.get(0, b"key-0"[..].into())?);
        assert_eq!(
            Some(b"val-42"[..].into()),
            cask.get(0, b"key-42"[..].into())?
        );
        assert_eq!(None, cask.get(0, b"key-100"[..].into())?);
        Ok(())
    }

    #[test]
    fn test_restore() -> Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        let tmp_pth = tmp_dir.path().join("mona.cask");
        // setup
        let mut cask = Cask::new(&tmp_pth)?;
        let num_in_bin_0: usize = 11;
        let num_in_bin_1: usize = 27;
        put_many(&mut cask, 0, num_in_bin_0)?;
        put_many(&mut cask, 1, num_in_bin_1)?;
        assert_eq!(
            num_in_bin_0,
            cask.count(0),
            "bin 0 should have {} records",
            num_in_bin_0
        );
        assert_eq!(
            num_in_bin_1,
            cask.count(1),
            "bin 1 should have {} records",
            num_in_bin_1
        );
        cask.close();
        // restore
        let restored = Cask::open(&tmp_pth)?;
        assert_eq!(
            num_in_bin_0,
            restored.count(0),
            "bin 0 should have {} records",
            num_in_bin_0
        );
        assert_eq!(
            num_in_bin_1,
            restored.count(1),
            "bin 1 should have {} records",
            num_in_bin_1
        );
        Ok(())
    }

    #[test]
    fn test_scan() -> Result<()> {
        // setup
        let mut cask = cask()?;
        cask.put(0, Bytes::from_static(b"k1"), Bytes::from_static(b"v1"))?;
        cask.put(0, Bytes::from_static(b"k2"), Bytes::from_static(b"v2"))?;
        cask.put(0, Bytes::from_static(b"k3"), Bytes::from_static(b"v3"))?;
        // scan
        let result = cask.scan(0)?;
        let expect = vec![
            Bytes::from_static(b"v1"),
            Bytes::from_static(b"v2"),
            Bytes::from_static(b"v3"),
        ];
        assert_eq!(expect, result);
        Ok(())
    }
}

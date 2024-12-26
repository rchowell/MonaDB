use std::collections::HashMap;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::{fs::File, path::Path};

use crate::{error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

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
        File::create(&path)?;
        Self::open(path)
    }

    pub fn open<P>(path: P) -> Result<Cask>
    where
        P: AsRef<Path>,
    {
        let bins = LogBins::new();
        let log_r = LogReader::open(&path)?;
        let log_w = LogWriter::open(&path)?;
        Cask { bins, log_r, log_w }.sync()
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
            // debug
            println!("ptr(pos={}, len={})", ptr.pos, ptr.len);
            let record = self.log_r.get(ptr)?;
            println!("{:?}", record);
            return Ok(Some(record.val));
        }
        Ok(None)
    }

    pub fn del(&mut self, _key: Bytes) -> Result<bool> {
        Ok(false)
    }

    pub fn close(self) {
        drop(self.log_w);
    }

    fn sync(self) -> Result<Self> {
        // let mut pos: u64 = 0;
        // let end: u64 = file.metadata()?.len();

        // while pos < end {
        //     // read header
        //     let mut k_size = [0u8; 2];
        //     let mut v_size = [0u8; 8];
        //     file.read_exact(&mut k_size)?;
        //     file.read_exact(&mut v_size)?;
        //     let k_size = u16::from_be_bytes(k_size);
        //     let v_size = u64::from_be_bytes(v_size);

        //     // read key
        //     let mut key = BytesMut::with_capacity(k_size as usize);
        //     key.resize(k_size as usize, 0);
        //     file.read_exact(&mut key)?;

        //     // debug
        //     // println!("k_size={},v_size={},key={:?}", k_size, v_size, &key);

        //     // map[key] = ptr
        //     let len = 10u64 + (k_size as u64) + v_size;
        //     let ptr = LogPtr { pos, len };
        //     map.insert(key.into(), ptr);

        //     // seek (has overflow bug)
        //     file.seek(SeekFrom::Current(len as i64))?;
        //     pos += len
        // }

        // Ok(())
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
            bins: Vec::from([NONE; 127]),
        }
    }

    pub fn count(&self, bin: usize) -> usize {
        self.bins[bin].as_ref().map_or(0, |m| m.len())
    }

    pub fn put(&mut self, bin: usize, key: Bytes, ptr: LogPtr) -> Result<()> {
        if bin > 127 {
            error!("bin cannot exceed 127")
        }
        if self.bins[bin].is_none() {
            self.bins[bin] = Some(LogBin::new());
        }
        self.bins[bin].as_mut().unwrap().insert(key, ptr);
        Ok(())
    }

    pub fn get(&self, bin: usize, key: Bytes) -> Result<Option<LogPtr>> {
        if bin > 127 {
            error!("bin cannot exceed 127")
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
        let f = File::options().read(true).open(&path)?;
        let r = LogReader::new(f)?;
        Ok(r)
    }

    pub fn get(&mut self, ptr: LogPtr) -> Result<LogRecord> {
        let mut buf = BytesMut::with_capacity(ptr.len as usize);
        buf.resize(ptr.len as usize, 0);
        self.log.seek(SeekFrom::Start(ptr.pos))?;
        self.log.read_exact(&mut buf)?;
        Ok(buf.freeze().into())
    }
}

#[derive(Debug, Clone, Copy)]
struct LogPtr {
    pos: u64,
    len: u64,
}

#[derive(Debug, Clone)]
struct LogRecord {
    bin: u8,
    key: Bytes,
    val: Bytes,
}

impl LogRecord {
    pub fn new(bin: usize, key: Bytes, val: Bytes) -> Result<Self> {
        if bin > 127 {
            error!("bin too large (max 127)")
        }
        if key.len() > (u8::MAX as usize) {
            error!("key too large (max 256b)")
        }
        if val.len() > (u16::MAX as usize) {
            error!("val too large (max 64kb)")
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
        // calculate size
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
        let bin = *value.first().unwrap();
        let n = value.slice(1..2).get_u8() as usize;
        let m = value.slice(2..4).get_u16() as usize;
        // body
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
    use bytes::{BufMut, BytesMut};

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
        assert_eq!(Some(b"val-42"[..].into()), cask.get(0, b"key-42"[..].into())?);
        assert_eq!(None, cask.get(0, b"key-100"[..].into())?);
        Ok(())
    }
}

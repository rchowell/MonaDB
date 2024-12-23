use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::{fs::File, path::Path};
use std::collections::HashMap;

use bytes::{BufMut, Bytes, BytesMut};
use crate::Result;

/// A single-file key-value store based on bitcask.
#[allow(dead_code)]
pub struct Cask {
    map: HashMap<Bytes, Pointer>,
    log_writer: Log<File>,
    log_reader: File,
}

#[allow(dead_code)]
impl Cask {

    pub fn open<P>(path: P) -> Result<Cask>
    where
        P: AsRef<Path>,
    {

        // hack to get another file descriptor.. 
        let mut log_reader = File::options().read(true).open(&path)?;
        let map = Self::restore(&mut log_reader)?;
        
        // write to this log
        let file = File::options().write(true).open(&path)?;
        let log_writer = Log::new(file)?;

        Ok(Cask { map, log_writer, log_reader })
    }
    
    pub fn get(&mut self, key: Bytes) -> Result<Option<Bytes>> {
        if let Some(ptr) = self.map.get(&key) {
            // debug
            // println!("ptr(pos={}, len={})", ptr.pos, ptr.len);

            // buffer
            let mut buf = BytesMut::with_capacity(ptr.len as usize);
            buf.resize(ptr.len as usize, 0);

            // read
            self.log_reader.seek(SeekFrom::Start(ptr.pos))?;
            self.log_reader.read_exact(&mut buf)?;

            // parse record
            let record: Record = buf.freeze().into();

            // extract value
            return Ok(Some(record.v))
        }
        Ok(None)
    }

    pub fn put(&mut self, key: Bytes, value: Bytes) -> Result<()> {
        // write to file
        let record = Record { k: key.clone(), v: value };
        let ptr = self.log_writer.append(record)?;
        // update keys
        self.map.insert(key, ptr);
        Ok(())
    }
    
    pub fn del(&mut self, _key: Bytes) -> Result<bool> {
        Ok(false)
    }

    pub fn close(self) {
        drop(self.log_writer);
    }

    fn restore(file: &mut File) -> Result<HashMap<Bytes, Pointer>> {

        let mut map = HashMap::<Bytes, Pointer>::new();
        let mut pos: u64 = 0;
        let end: u64 = file.metadata()?.len();

        while pos < end {

            // read header
            let mut k_size = [0u8;2];
            let mut v_size = [0u8;8];
            file.read_exact(&mut k_size)?;
            file.read_exact(&mut v_size)?;
            let k_size = u16::from_be_bytes(k_size);
            let v_size = u64::from_be_bytes(v_size);


            // read key
            let mut key = BytesMut::with_capacity(k_size as usize);
            key.resize(k_size as usize, 0);
            file.read_exact(&mut key)?;

            // debug
            // println!("k_size={},v_size={},key={:?}", k_size, v_size, &key);

            // map[key] = ptr
            let len = 10u64 + (k_size as u64) + v_size;
            let ptr = Pointer { pos, len };
            map.insert(key.into(), ptr);

            // seek (has overflow bug)
            file.seek(SeekFrom::Current(len as i64))?;
            pos += len
        }

        Ok(map)
    }
}

/// A writeable log.
struct Log<W: Write> {
    pos: u64,
    log: BufWriter<W>,
}

impl<W> Log<W> where W: Write + Seek {

    pub fn new(mut w: W) -> std::io::Result<Self> {
        let pos = w.seek(SeekFrom::End(0))?;
        let log = BufWriter::new(w);
        Ok(Log{ pos, log })
    }

    pub fn append(&mut self, record: Record) -> Result<Pointer> {
        // create buffer
        let pos = self.pos;
        let buf: Bytes = record.into();
        // append to file
        self.pos += self.log.write(&buf)? as u64;
        // TODO remove me
        self.log.flush()?;
        // done
        Ok(Pointer { pos, len: self.pos - pos })
    }
}

/// A record (log entry) in the cask file.
/// Lots of routes to optimize read-writing of records, just bytes for now...
struct Record {
    k: Bytes,
    v: Bytes,
}

/// Write a record as bytes (without bincode).
/// 
/// [[ ksize ][ vsize ][ key ][ value ]]
///     u16      u64
/// 
impl From<Record> for Bytes {

    fn from(val: Record) -> Self {
        let k_size: usize = val.k.len();
        let v_size: usize = val.v.len();
        let mut buf = BytesMut::with_capacity(k_size + v_size);
        buf.put_u16(k_size as u16);
        buf.put_u64(v_size as u64);
        buf.put(val.k);
        buf.put(val.v);
        buf.into()
    }
}

impl From<Bytes> for Record {

    fn from(value: Bytes) -> Self {
        // header
        let k_size_buf: [u8; 2] = value[0..2].try_into().unwrap();
        let v_size_buf: [u8; 8] = value[2..10].try_into().unwrap();
        let k_size = u16::from_be_bytes(k_size_buf) as usize;
        let v_size = u64::from_be_bytes(v_size_buf) as usize;
        // body
        let h_o = 10;
        let k_o = h_o + k_size;
        let v_o = k_o + v_size;
        let k = value.slice(h_o..k_o);
        let v = value.slice(k_o..v_o);
        Record { k, v }
    }
}

// A pointer to a record in the cask file.
struct Pointer {
    pos: u64,
    len: u64,
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};
    use crate::{cask::Cask, Result};

    #[test]
    fn test_write() -> Result<()> {
        // make path
        let tmp_dir= tempfile::tempdir()?;
        let tmp_pth = tmp_dir.path().join("mona.cask");
        File::create(&tmp_pth)?;

        // open cask
        let mut cask = Cask::open(&tmp_pth)?;

        // key-value pair
        let k = b"hello".as_slice();
        let v = b"world".as_slice();

        // put a value
        cask.put(k.into(), v.into())?;

        // close
        cask.close();

        // assert 20 bytes are written for [[5][5][hello][world]]
        let f = File::open(&tmp_pth)?;
        let s = f.metadata()?.len();

        assert_eq!(20, s, "sizeof(cask) = 20, found {}", s);
        Ok(())
    }

    #[test]
    fn test_read() -> Result<()> {
        // make file
        let tmp_dir= tempfile::tempdir().unwrap();
        let tmp_pth = tmp_dir.path().join("mona.cask");

        // write known bytes
        let mut file = File::create(&tmp_pth)?;
        let bytes: Vec<u8> = vec![
            0x00, 0x05,                                         // 5 (u16)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,     // 5 (u64)
            0x68, 0x65, 0x6c, 0x6c, 0x6f,                       // hello
            0x77, 0x6f, 0x72, 0x6c, 0x64                        // world
        ];
        file.write_all(&bytes)?;

        // open the cask
        let mut cask = Cask::open(&tmp_pth)?;

        // key-value pair
        let k = b"hello".as_slice();
        let v = b"world".as_slice();

        let res = cask.get(k.into())?;
        
        // check
        assert_eq!(res, Some(v.into()), "expected 'world', got {:?}", res);

        Ok(())
    }

}

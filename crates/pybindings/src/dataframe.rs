use std::io::Cursor;

use polars::io::{SerReader, SerWriter};
use polars::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule};

pub fn df_from_py(obj: &Bound<'_, PyAny>) -> PyResult<DataFrame> {
    let py = obj.py();
    let io = PyModule::import(py, "io")?;
    let buf = io.getattr("BytesIO")?.call0()?;
    let method = if obj.hasattr("write_ipc")? {
        "write_ipc"
    } else {
        "to_ipc"
    };
    obj.call_method1(method, (buf.clone(),))?;
    let data: Vec<u8> = buf.call_method0("getvalue")?.extract()?;

    let df = IpcReader::new(Cursor::new(data))
        .finish()
        .map_err(|e| PyValueError::new_err(format!("failed to read Arrow IPC from DataFrame: {e}")))?;
    Ok(df)
}

pub fn df_to_py(py: Python<'_>, df: &DataFrame) -> PyResult<Py<PyAny>> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut out = df.clone();
        out.clear_schema();
        let mut writer = IpcWriter::new(&mut buf);
        writer
            .finish(&mut out)
            .map_err(|e| PyValueError::new_err(format!("failed to write Arrow IPC: {e}")))?;
    }

    let pl = PyModule::import(py, "polars")?;
    let io = PyModule::import(py, "io")?;
    let bytesio = io
        .getattr("BytesIO")?
        .call1((PyBytes::new(py, &buf),))?;
    Ok(pl.getattr("read_ipc")?.call1((bytesio,))?.unbind())
}

/// A fresh empty `polars.DataFrame`, used as the default output frame.
pub fn empty_df_py(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let pl = PyModule::import(py, "polars")?;
    Ok(pl.call_method0("DataFrame")?.unbind())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipc_roundtrip_keeps_renamed_columns() {
        let df = DataFrame::new(vec![
            Column::new("date".into(), &[19723i32, 19724, 19725]),
            Column::new("close".into(), &[1.0f64, 2.0, 3.0]),
        ])
        .unwrap();

        let mask = BooleanChunked::from_slice("mask".into(), &[true, true, true]);
        let mut chunk = df.filter(&mask).unwrap();
        chunk.rename("close", "price".into()).unwrap();
        assert_eq!(chunk.get_column_names(), &["date", "price"]);

        let mut buf: Vec<u8> = Vec::new();
        {
            let mut out = chunk.clone();
            out.clear_schema();
            let mut writer = IpcWriter::new(&mut buf);
            writer.finish(&mut out).unwrap();
        }

        let reread = IpcReader::new(Cursor::new(&buf)).finish().unwrap();
        assert_eq!(
            reread.get_column_names(),
            &["date", "price"],
            "IPC round trip lost the rename"
        );
    }
}

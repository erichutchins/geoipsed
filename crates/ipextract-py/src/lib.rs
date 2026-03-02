#![allow(clippy::useless_conversion)]

use ip_extract::ExtractorBuilder;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyString};

/// Extract bytes from a Python str or bytes object.
// Perf note: this copies the input. PyO3 0.28's PyBackedStr could avoid the copy
// via zero-copy borrow, but the one-time copy is negligible vs DFA scanning cost.
fn as_bytes(text: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(s) = text.cast::<PyString>() {
        Ok(s.to_str()?.as_bytes().to_vec())
    } else if let Ok(b) = text.cast::<PyBytes>() {
        Ok(b.as_bytes().to_vec())
    } else {
        Err(PyValueError::new_err("expected str or bytes"))
    }
}

#[pyclass(name = "Extractor")]
struct PyExtractor {
    inner: ip_extract::Extractor,
    include_ipv4: bool,
    include_ipv6: bool,
    include_private: bool,
    include_loopback: bool,
    include_broadcast: bool,
}

impl PyExtractor {
    fn from_config(
        ipv4: bool,
        ipv6: bool,
        private: bool,
        loopback: bool,
        broadcast: bool,
    ) -> PyResult<Self> {
        let mut builder = ExtractorBuilder::new();
        builder.ipv4(ipv4);
        builder.ipv6(ipv6);
        builder.private_ips(private);
        builder.loopback_ips(loopback);
        builder.broadcast_ips(broadcast);
        let inner = builder
            .build()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner,
            include_ipv4: ipv4,
            include_ipv6: ipv6,
            include_private: private,
            include_loopback: loopback,
            include_broadcast: broadcast,
        })
    }
}

#[pymethods]
impl PyExtractor {
    #[new]
    #[pyo3(signature = (*, private=true, loopback=true, broadcast=true, ipv4=true, ipv6=true))]
    fn new(
        private: bool,
        loopback: bool,
        broadcast: bool,
        ipv4: bool,
        ipv6: bool,
    ) -> PyResult<Self> {
        Self::from_config(ipv4, ipv6, private, loopback, broadcast)
    }

    fn extract(&self, text: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
        let bytes = as_bytes(text)?;
        Ok(self
            .inner
            .match_iter(&bytes)
            .map(|m| m.as_str_refanged().into_owned())
            .collect())
    }

    fn extract_unique(&self, text: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
        use std::collections::HashSet;
        let bytes = as_bytes(text)?;
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for m in self.inner.match_iter(&bytes) {
            let ip = m.as_str_refanged().into_owned();
            if seen.insert(ip.clone()) {
                result.push(ip);
            }
        }
        Ok(result)
    }

    fn extract_with_offsets(
        &self,
        text: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<(String, usize, usize)>> {
        let bytes = as_bytes(text)?;
        Ok(self
            .inner
            .match_iter(&bytes)
            .map(|m| {
                let r = m.range();
                (m.as_str_refanged().into_owned(), r.start, r.end)
            })
            .collect())
    }

    fn only_public(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(self.include_ipv4, self.include_ipv6, false, false, false)?,
        )
    }

    fn ignore_private(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                self.include_ipv4,
                self.include_ipv6,
                false,
                self.include_loopback,
                self.include_broadcast,
            )?,
        )
    }

    fn ignore_loopback(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                self.include_ipv4,
                self.include_ipv6,
                self.include_private,
                false,
                self.include_broadcast,
            )?,
        )
    }

    fn ignore_broadcast(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                self.include_ipv4,
                self.include_ipv6,
                self.include_private,
                self.include_loopback,
                false,
            )?,
        )
    }

    #[pyo3(signature = (include))]
    fn ipv4(&self, py: Python<'_>, include: bool) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                include,
                self.include_ipv6,
                self.include_private,
                self.include_loopback,
                self.include_broadcast,
            )?,
        )
    }

    #[pyo3(signature = (include))]
    fn ipv6(&self, py: Python<'_>, include: bool) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                self.include_ipv4,
                include,
                self.include_private,
                self.include_loopback,
                self.include_broadcast,
            )?,
        )
    }

    #[pyo3(signature = (include))]
    fn private_ips(&self, py: Python<'_>, include: bool) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                self.include_ipv4,
                self.include_ipv6,
                include,
                self.include_loopback,
                self.include_broadcast,
            )?,
        )
    }

    #[pyo3(signature = (include))]
    fn loopback_ips(&self, py: Python<'_>, include: bool) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                self.include_ipv4,
                self.include_ipv6,
                self.include_private,
                include,
                self.include_broadcast,
            )?,
        )
    }

    #[pyo3(signature = (include))]
    fn broadcast_ips(&self, py: Python<'_>, include: bool) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_config(
                self.include_ipv4,
                self.include_ipv6,
                self.include_private,
                self.include_loopback,
                include,
            )?,
        )
    }
}

#[pyfunction]
fn extract(text: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let bytes = as_bytes(text)?;
    let extractor = ExtractorBuilder::new()
        .build()
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(extractor
        .match_iter(&bytes)
        .map(|m| m.as_str_refanged().into_owned())
        .collect())
}

#[pyfunction]
fn extract_unique(text: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    use std::collections::HashSet;
    let bytes = as_bytes(text)?;
    let extractor = ExtractorBuilder::new()
        .build()
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for m in extractor.match_iter(&bytes) {
        let ip = m.as_str_refanged().into_owned();
        if seen.insert(ip.clone()) {
            result.push(ip);
        }
    }
    Ok(result)
}

#[pymodule]
fn _ipextract(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyExtractor>()?;
    m.add_function(wrap_pyfunction!(extract, m)?)?;
    m.add_function(wrap_pyfunction!(extract_unique, m)?)?;
    Ok(())
}

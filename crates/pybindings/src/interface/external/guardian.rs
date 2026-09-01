//! Guardian Content API bindings.

use pyo3::prelude::*;

use ::dxcore::interface::external::guardian::{
    get_article, search, GuardianArticle, GuardianArticleBody, GuardianBlock,
};

use super::to_py_err;

#[pyclass(name = "GuardianArticle", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyGuardianArticle {
    inner: GuardianArticle,
}

#[pymethods]
impl PyGuardianArticle {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn article_type(&self) -> &str {
        &self.inner.article_type
    }

    #[getter]
    fn section_id(&self) -> Option<&str> {
        self.inner.section_id.as_deref()
    }

    #[getter]
    fn section_name(&self) -> Option<&str> {
        self.inner.section_name.as_deref()
    }

    #[getter]
    fn web_publication_date(&self) -> &str {
        &self.inner.web_publication_date
    }

    #[getter]
    fn web_title(&self) -> &str {
        &self.inner.web_title
    }

    #[getter]
    fn web_url(&self) -> &str {
        &self.inner.web_url
    }

    #[getter]
    fn api_url(&self) -> &str {
        &self.inner.api_url
    }

    #[getter]
    fn pillar_name(&self) -> Option<&str> {
        self.inner.pillar_name.as_deref()
    }
}

#[pyclass(name = "GuardianBlock", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyGuardianBlock {
    inner: GuardianBlock,
}

#[pymethods]
impl PyGuardianBlock {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn body_html(&self) -> &str {
        &self.inner.body_html
    }

    #[getter]
    fn body_text_summary(&self) -> &str {
        &self.inner.body_text_summary
    }
}

#[pyclass(name = "GuardianArticleBody", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyGuardianArticleBody {
    inner: GuardianArticleBody,
}

#[pymethods]
impl PyGuardianArticleBody {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn web_publication_date(&self) -> &str {
        &self.inner.web_publication_date
    }

    #[getter]
    fn web_title(&self) -> &str {
        &self.inner.web_title
    }

    #[getter]
    fn web_url(&self) -> &str {
        &self.inner.web_url
    }

    #[getter]
    fn blocks(&self) -> Vec<PyGuardianBlock> {
        self.inner
            .blocks
            .as_ref()
            .and_then(|b| b.body.as_ref())
            .map(|blocks| {
                blocks
                    .iter()
                    .cloned()
                    .map(|inner| PyGuardianBlock { inner })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[pyclass(name = "GuardianClient", module = "dxcore")]
pub struct PyGuardianClient {
    api_key: String,
}

#[pymethods]
impl PyGuardianClient {
    #[new]
    fn new(api_key: String) -> Self {
        Self { api_key }
    }

    /// Search articles, newest first.
    #[pyo3(signature = (query, page_size=10))]
    fn search(&self, query: &str, page_size: usize) -> PyResult<Vec<PyGuardianArticle>> {
        search(query, &self.api_key, page_size)
            .map(|rows| {
                rows.into_iter()
                    .map(|inner| PyGuardianArticle { inner })
                    .collect()
            })
            .map_err(to_py_err)
    }

    /// Fetch one article, including its body blocks.
    fn get_article(&self, id: &str) -> PyResult<PyGuardianArticleBody> {
        get_article(id, &self.api_key)
            .map(|inner| PyGuardianArticleBody { inner })
            .map_err(to_py_err)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGuardianClient>()?;
    m.add_class::<PyGuardianArticle>()?;
    m.add_class::<PyGuardianBlock>()?;
    m.add_class::<PyGuardianArticleBody>()?;
    Ok(())
}

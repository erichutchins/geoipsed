use anyhow::Result;
use camino::Utf8PathBuf;
use std::net::IpAddr;
use termcolor::ColorChoice;

use crate::mmdb::{MmdbProvider, ProviderRegistry};
use crate::template::Template;

// Default template format
const DEFAULT_TEMPLATE: &str = "<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>";

pub struct GeoIPSed {
    pub color: ColorChoice,
    template: Template,
    pub only_routable: bool,
    pub provider: Box<dyn MmdbProvider>,
}

impl GeoIPSed {
    pub fn new(
        mmdbpath: Option<Utf8PathBuf>,
        user_template: Option<String>,
        color: ColorChoice,
        only_routable: bool,
    ) -> Result<Self> {
        let mut registry = ProviderRegistry::default();
        registry.initialize_active_provider(mmdbpath)?;
        let provider = registry.get_active_provider_owned()?;

        let template_str = if color == ColorChoice::Always {
            format!(
                "\x1b[1;31m{}\x1b[0;0m",
                user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
            )
        } else {
            user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
        };

        let template = Template::compile(&template_str)
            .map_err(|e| anyhow::anyhow!("Invalid template: {}", e))?;

        Ok(Self {
            color,
            template,
            only_routable,
            provider,
        })
    }

    #[inline]
    pub fn new_with_provider(
        _mmdbpath: Option<Utf8PathBuf>,
        user_template: Option<String>,
        color: ColorChoice,
        only_routable: bool,
        mut provider_registry: ProviderRegistry,
    ) -> Result<Self> {
        let provider = provider_registry.get_active_provider_owned()?;

        let template_str = if color == ColorChoice::Always {
            // if we are printing color, bookend the template with ansi red escapes
            format!(
                "\x1b[1;31m{}\x1b[0;0m",
                user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
            )
        } else {
            user_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string())
        };

        // Compile the template once during initialization
        let template = Template::compile(&template_str)
            .map_err(|e| anyhow::anyhow!("Invalid template: {}", e))?;

        Ok(Self {
            color,
            template,
            only_routable,
            provider,
        })
    }

    #[inline]
    pub fn lookup(&self, ip: IpAddr, s: &str) -> String {
        // Only proceed with routability check if needed
        if self.only_routable && !self.provider.has_asn(ip) {
            return s.to_string();
        }

        // Use the provider for lookup
        if let Ok(result) = self.provider.lookup(ip, s, &self.template) {
            return result;
        }

        s.to_string()
    }

    #[inline]
    pub fn lookup_and_write<W: std::io::Write>(
        &self,
        wtr: &mut W,
        ip: IpAddr,
        s: &str,
    ) -> Result<()> {
        if self.only_routable && !self.provider.has_asn(ip) {
            wtr.write_all(s.as_bytes())?;
            return Ok(());
        }

        if self
            .provider
            .lookup_and_write(wtr, ip, s, &self.template)
            .is_err()
        {
            wtr.write_all(s.as_bytes())?;
        }

        Ok(())
    }
}

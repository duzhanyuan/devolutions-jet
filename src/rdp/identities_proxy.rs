use std::{fs::File, io, io::prelude::*};

use serde_derive::{Deserialize, Serialize};

pub trait RdpIdentityGetter {
    fn get_rdp_identity(&self) -> RdpIdentity;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RdpIdentity {
    pub proxy: rdp_proto::Credentials,
    pub target: rdp_proto::Credentials,
    pub destination: String,
}

pub struct IdentitiesProxy {
    pub rdp_identity: Option<RdpIdentity>,
    rdp_identities_filename: String,
}

impl RdpIdentity {
    fn from_file(filename: &str) -> io::Result<Vec<Self>> {
        let mut f = File::open(filename)?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;

        Ok(serde_json::from_str(&contents).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to read the json data: {}", e),
            )
        })?)
    }
}

impl IdentitiesProxy {
    pub fn new(rdp_identities_filename: String) -> Self {
        Self {
            rdp_identities_filename,
            rdp_identity: None,
        }
    }
}

impl RdpIdentityGetter for IdentitiesProxy {
    fn get_rdp_identity(&self) -> RdpIdentity {
        self.rdp_identity
            .as_ref()
            .expect("RdpIdentity must be set before the call")
            .clone()
    }
}

impl rdp_proto::CredentialsProxy for IdentitiesProxy {
    fn password_by_user(&mut self, username: String, _domain: Option<String>) -> io::Result<String> {
        let rdp_identities = RdpIdentity::from_file(self.rdp_identities_filename.as_ref())?;
        let rdp_identity = rdp_identities
            .iter()
            .find(|identity| identity.proxy.username == username);

        if let Some(identity) = rdp_identity {
            self.rdp_identity = Some(identity.clone());
            Ok(identity.proxy.password.clone())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("failed to find identity with the username '{}'", username),
            ))
        }
    }
}

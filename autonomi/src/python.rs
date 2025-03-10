use std::{path::PathBuf, str::FromStr, sync::Arc};

use crate::{
    client::{
        chunk::DataMapChunk,
        payment::PaymentOption,
        vault::{UserData, VaultSecretKey},
    },
    files::{Metadata, PrivateArchive, PublicArchive},
    register::{RegisterAddress, RegisterHistory},
    Client, ClientConfig,
};
use crate::{Bytes, Network, Wallet};
use ant_protocol::storage::{
    Chunk, ChunkAddress, GraphEntry, GraphEntryAddress, Pointer, PointerAddress, PointerTarget,
    Scratchpad, ScratchpadAddress,
};
use bls::{PublicKey, SecretKey};
use libp2p::Multiaddr;
use pyo3::exceptions::{PyConnectionError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use xor_name::XorName;

/// Represents a client for the Autonomi network.
// Missing methods:
// - upload_chunks_with_retries
// - enable_client_events
// - evm_network
// - get_store_quotes
// - pointer_verify
// - scratchpad_verify
#[pyclass(name = "Client")]
pub(crate) struct PyClient {
    inner: Client,
}

#[pymethods]
impl PyClient {
    /// Initialize the client with default configuration.
    #[staticmethod]
    fn init(py: Python) -> PyResult<Bound<PyAny>> {
        future_into_py(py, async {
            let inner = Client::init()
                .await
                .map_err(|e| PyConnectionError::new_err(format!("Failed to connect: {e}")))?;
            Ok(PyClient { inner })
        })
    }

    /// Initialize a client that is configured to be local.
    #[staticmethod]
    fn init_local(py: Python) -> PyResult<Bound<PyAny>> {
        future_into_py(py, async {
            let inner = Client::init_local()
                .await
                .map_err(|e| PyConnectionError::new_err(format!("Failed to connect: {e}")))?;
            Ok(PyClient { inner })
        })
    }

    /// Initialize a client that bootstraps from a list of peers.
    ///
    /// If any of the provided peers is a global address, the client will not be local.
    #[staticmethod]
    fn init_with_peers(py: Python, peers: Vec<String>) -> PyResult<Bound<PyAny>> {
        let peers: Vec<Multiaddr> = peers
            .iter()
            .map(|p| Multiaddr::from_str(p))
            .collect::<Result<_, _>>()
            .map_err(|e| PyValueError::new_err(format!("Failed to parse peers: {e}")))?;

        future_into_py(py, async {
            let inner = Client::init_with_peers(peers)
                .await
                .map_err(|e| PyConnectionError::new_err(format!("Failed to connect: {e}")))?;
            Ok(PyClient { inner })
        })
    }

    /// Initialize the client with the given configuration.
    #[staticmethod]
    fn init_with_config(py: Python, config: PyClientConfig) -> PyResult<Bound<PyAny>> {
        future_into_py(py, async {
            let inner = Client::init_with_config(config.inner)
                .await
                .map_err(|e| PyConnectionError::new_err(format!("Failed to connect: {e}")))?;
            Ok(PyClient { inner })
        })
    }

    /// Get the cost of storing a chunk on the network
    fn chunk_cost<'a>(&self, py: Python<'a>, addr: PyChunkAddress) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client
                .chunk_cost(&addr.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get chunk cost: {e}")))?;
            Ok(cost.to_string())
        })
    }

    /// Get a chunk from the network.
    fn chunk_get<'a>(&self, py: Python<'a>, addr: &PyChunkAddress) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let addr = addr.inner;

        future_into_py(py, async move {
            let chunk = client
                .chunk_get(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get chunk: {e}")))?;
            Ok(chunk.value.to_vec())
        })
    }

    /// Manually upload a chunk to the network. It is recommended to use the `data_put` method instead to upload data.
    fn chunk_put<'a>(
        &self,
        py: Python<'a>,
        data: Vec<u8>,
        payment: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment.inner.clone();
        let chunk = Chunk::new(Bytes::from(data));

        future_into_py(py, async move {
            let (cost, addr) = client
                .chunk_put(&chunk, payment)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to put chunk: {e}")))?;
            Ok((cost.to_string(), PyChunkAddress::from(addr)))
        })
    }

    /// Fetches a GraphEntry from the network.
    fn graph_entry_get<'a>(
        &self,
        py: Python<'a>,
        #[pyo3(from_py_with = "str_to_addr")] addr: XorName,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let addr = GraphEntryAddress(addr);

        future_into_py(py, async move {
            let entry = client
                .graph_entry_get(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get graph entry: {e}")))?;
            Ok(PyGraphEntry { inner: entry })
        })
    }

    /// Check if a graph_entry exists on the network
    fn graph_entry_check_existance<'a>(
        &self,
        py: Python<'a>,
        #[pyo3(from_py_with = "str_to_addr")] addr: XorName,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let addr = GraphEntryAddress(addr);

        future_into_py(py, async move {
            let exists = client
                .graph_entry_check_existance(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get graph entry: {e}")))?;
            Ok(exists)
        })
    }

    /// Manually puts a GraphEntry to the network.
    fn graph_entry_put<'a>(
        &self,
        py: Python<'a>,
        entry: PyGraphEntry,
        payment_option: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment_option.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .graph_entry_put(entry.inner, payment)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get graph entry: {e}")))?;

            Ok((
                cost.to_string(),
                crate::client::address::addr_to_str(addr.0),
            ))
        })
    }

    /// Get the cost to create a GraphEntry
    fn graph_entry_cost<'a>(&self, py: Python<'a>, key: PyPublicKey) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client.graph_entry_cost(&key.inner).await.map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to get graph entry cost: {e}"))
            })?;

            Ok(cost.to_string())
        })
    }

    /// Get Scratchpad from the Network.
    /// A Scratchpad is stored at the owner's public key so we can derive the address from it.
    fn scratchpad_get_from_public_key<'a>(
        &self,
        py: Python<'a>,
        public_key: PyPublicKey,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let scratchpad = client
                .scratchpad_get_from_public_key(&public_key.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get scratchpad: {e}")))?;

            Ok(PyScratchpad { inner: scratchpad })
        })
    }

    /// Get Scratchpad from the Network using the scratpad address in hex string format.
    fn scratchpad_get<'a>(&self, py: Python<'a>, addr: String) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let addr = ScratchpadAddress::from_hex(&addr)
            .map_err(|e| PyValueError::new_err(format!("Failed to parse address: {e}")))?;

        future_into_py(py, async move {
            let scratchpad = client
                .scratchpad_get(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get scratchpad: {e}")))?;

            Ok(PyScratchpad { inner: scratchpad })
        })
    }

    /// Check if a scratchpad exists on the network
    fn scratchpad_check_existance<'a>(
        &self,
        py: Python<'a>,
        addr: String,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let addr = ScratchpadAddress::from_hex(&addr)
            .map_err(|e| PyValueError::new_err(format!("Failed to parse address: {e}")))?;

        future_into_py(py, async move {
            let exists = client
                .scratchpad_check_existance(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get scratchpad: {e}")))?;

            Ok(exists)
        })
    }

    /// Manually store a scratchpad on the network
    fn scratchpad_put<'a>(
        &self,
        py: Python<'a>,
        scratchpad: PyScratchpad,
        payment_option: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment_option.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .scratchpad_put(scratchpad.inner, payment)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to put scratchpad: {e}")))?;

            Ok((cost.to_string(), addr.to_hex()))
        })
    }

    /// Create a new scratchpad to the network.
    ///
    /// Make sure that the owner key is not already used for another scratchpad as each key is associated with one scratchpad.
    /// The data will be encrypted with the owner key before being stored on the network.
    /// The content type is used to identify the type of data stored in the scratchpad, the choice is up to the caller.
    ///
    /// Returns the cost and the address of the scratchpad.
    fn scratchpad_create<'a>(
        &self,
        py: Python<'a>,
        owner: PySecretKey,
        content_type: u64,
        initial_data: Vec<u8>,
        payment_option: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment_option.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .scratchpad_create(
                    &owner.inner,
                    content_type,
                    &Bytes::from(initial_data),
                    payment,
                )
                .await
                .map_err(|e| {
                    PyRuntimeError::new_err(format!("Failed to create scratchpad: {e}"))
                })?;

            Ok((cost.to_string(), addr.to_hex()))
        })
    }

    /// Update an existing scratchpad to the network.
    /// The scratchpad needs to be created first with `scratchpad_create`.
    /// This operation is free as the scratchpad was already paid for at creation.
    /// Only the latest version of the scratchpad is kept on the Network, previous versions will be overwritten and unrecoverable.
    fn scratchpad_update<'a>(
        &self,
        py: Python<'a>,
        owner: PySecretKey,
        content_type: u64,
        data: Vec<u8>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            client
                .scratchpad_update(&owner.inner, content_type, &Bytes::from(data))
                .await
                .map_err(|e| {
                    PyRuntimeError::new_err(format!("Failed to update scratchpad: {e}"))
                })?;

            Ok(())
        })
    }

    /// Get the cost of creating a new Scratchpad
    fn scratchpad_cost<'a>(
        &self,
        py: Python<'a>,
        public_key: PyPublicKey,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client
                .scratchpad_cost(&public_key.inner)
                .await
                .map_err(|e| {
                    PyRuntimeError::new_err(format!("Failed to get scratchpad cost: {e}"))
                })?;

            Ok(cost.to_string())
        })
    }

    /// Get the cost of storing an archive on the network
    fn archive_cost<'a>(
        &self,
        py: Python<'a>,
        archive: PyPublicArchive,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client
                .archive_cost(&archive.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get archive cost: {e}")))?;
            Ok(cost.to_string())
        })
    }

    /// Fetch a private archive from the network using its data map
    fn archive_get<'a>(
        &self,
        py: Python<'a>,
        data_map: &PyDataMapChunk,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let data_map = data_map.inner.clone();

        future_into_py(py, async move {
            let archive = client
                .archive_get(&data_map)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get archive: {e}")))?;

            Ok(PyPrivateArchive { inner: archive })
        })
    }

    /// Upload a private archive to the network
    fn archive_put<'a>(
        &self,
        py: Python<'a>,
        archive: PyPrivateArchive,
        payment: PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let (cost, data_map) = client
                .archive_put(&archive.inner, payment.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to put archive: {e}")))?;

            Ok((cost.to_string(), PyDataMapChunk { inner: data_map }))
        })
    }

    /// Upload a public archive to the network
    fn archive_put_public<'a>(
        &self,
        py: Python<'a>,
        archive: PyPublicArchive,
        wallet: PyWallet,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .archive_put_public(&archive.inner, &wallet.inner)
                .await
                .map_err(|e| {
                    PyRuntimeError::new_err(format!("Failed to put public archive: {e}"))
                })?;

            Ok((cost.to_string(), crate::client::address::addr_to_str(addr)))
        })
    }

    /// Get the cost to upload a file/dir to the network.
    fn file_cost<'a>(&self, py: Python<'a>, path: PathBuf) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client
                .file_cost(&path)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get file cost: {e}")))?;

            Ok(cost.to_string())
        })
    }

    /// Download a private file from network to local file system.
    fn file_download<'a>(
        &self,
        py: Python<'a>,
        data_map: PyDataMapChunk,
        path: PathBuf,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            client
                .file_download(&data_map.inner, path)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to download file: {e}")))?;

            Ok(())
        })
    }

    /// Download a private directory from network to local file system
    fn dir_download<'a>(
        &self,
        py: Python<'a>,
        data_map: PyDataMapChunk,
        dir_path: PathBuf,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            client
                .dir_download(&data_map.inner, dir_path)
                .await
                .map_err(|e| {
                    PyRuntimeError::new_err(format!("Failed to download directory: {e}"))
                })?;
            Ok(())
        })
    }

    /// Upload a directory to the network. The directory is recursively walked and each file is uploaded to the network.
    /// The data maps of these (private) files are not uploaded but returned within the PrivateArchive return type.
    fn dir_upload<'a>(
        &self,
        py: Python<'a>,
        dir_path: PathBuf,
        wallet: PyWallet,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let (cost, archive) = client
                .dir_upload(dir_path, &wallet.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to upload directory: {e}")))?;
            Ok((cost.to_string(), PyPrivateArchive { inner: archive }))
        })
    }

    /// Download file from network to local file system.
    fn file_download_public<'a>(
        &self,
        py: Python<'a>,
        #[pyo3(from_py_with = "str_to_addr")] addr: XorName,
        path: PathBuf,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            client
                .file_download_public(&addr, path)
                .await
                .map_err(|e| {
                    PyRuntimeError::new_err(format!("Failed to download public file: {e}"))
                })?;

            Ok(())
        })
    }

    /// Same as `dir_upload` but also uploads the archive (privately) to the network.
    ///
    /// Returns the data map allowing the private archive to be downloaded from the network.
    fn dir_and_archive_upload<'a>(
        &self,
        py: Python<'a>,
        dir_path: PathBuf,
        wallet: PyWallet,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let (cost, data_map) = client
                .dir_and_archive_upload(dir_path, &wallet.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to upload directory: {e}")))?;
            Ok((cost.to_string(), PyDataMapChunk { inner: data_map }))
        })
    }

    /// Upload a piece of private data to the network. This data will be self-encrypted.
    /// The [`DataMapChunk`] is not uploaded to the network, keeping the data private.
    ///
    /// Returns the [`DataMapChunk`] containing the map to the encrypted chunks.
    fn data_put<'a>(
        &self,
        py: Python<'a>,
        data: Vec<u8>,
        payment: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment.inner.clone();

        future_into_py(py, async move {
            let (cost, data_map) = client
                .data_put(Bytes::from(data), payment)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to put data: {e}")))?;
            Ok((cost.to_string(), PyDataMapChunk { inner: data_map }))
        })
    }

    /// Fetch a blob of (private) data from the network
    fn data_get<'a>(&self, py: Python<'a>, access: &PyDataMapChunk) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let access = access.inner.clone();

        future_into_py(py, async move {
            let data = client
                .data_get(&access)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get data: {e}")))?;
            Ok(data.to_vec())
        })
    }

    /// Get the estimated cost of storing a piece of data.
    fn data_cost<'a>(&self, py: Python<'a>, data: Vec<u8>) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client
                .data_cost(Bytes::from(data))
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get data cost: {e}")))?;
            Ok(cost.to_string())
        })
    }

    /// Upload a piece of data to the network. This data is publicly accessible.
    ///
    /// Returns the Data Address at which the data was stored.
    fn data_put_public<'a>(
        &self,
        py: Python<'a>,
        data: Vec<u8>,
        payment: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .data_put_public(bytes::Bytes::from(data), payment)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to put data: {e}")))?;

            Ok((cost.to_string(), crate::client::address::addr_to_str(addr)))
        })
    }

    /// Fetch a blob of data from the network
    fn data_get_public<'a>(
        &self,
        py: Python<'a>,
        #[pyo3(from_py_with = "str_to_addr")] addr: XorName,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let data = client
                .data_get_public(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get data: {e}")))?;
            Ok(data.to_vec())
        })
    }

    /// Upload a directory as a public archive to the network.
    /// Returns the network address where the archive is stored.
    fn dir_and_archive_upload_public<'a>(
        &self,
        py: Python<'a>,
        dir_path: PathBuf,
        wallet: &PyWallet,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let wallet = wallet.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .dir_and_archive_upload_public(dir_path, &wallet)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to upload directory: {e}")))?;
            Ok((cost.to_string(), crate::client::address::addr_to_str(addr)))
        })
    }

    /// Download a public archive from the network to a local directory.
    fn dir_download_public<'a>(
        &self,
        py: Python<'a>,
        #[pyo3(from_py_with = "str_to_addr")] addr: XorName,
        dir_path: PathBuf,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            client
                .dir_download_public(&addr, dir_path)
                .await
                .map_err(|e| {
                    PyRuntimeError::new_err(format!("Failed to download directory: {e}"))
                })?;
            Ok(())
        })
    }

    /// Upload a directory to the network. The directory is recursively walked and each file is uploaded to the network.
    ///
    /// The data maps of these files are uploaded on the network, making the individual files publicly available.
    ///
    /// This returns, but does not upload (!),the `PublicArchive` containing the data maps of the uploaded files.
    fn dir_upload_public<'a>(
        &self,
        py: Python<'a>,
        dir_path: PathBuf,
        wallet: PyWallet,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let (cost, archive) = client
                .dir_upload_public(dir_path, &wallet.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to upload directory: {e}")))?;
            Ok((cost.to_string(), PyPublicArchive { inner: archive }))
        })
    }

    /// Get a public archive from the network.
    fn archive_get_public<'a>(
        &self,
        py: Python<'a>,
        #[pyo3(from_py_with = "str_to_addr")] addr: XorName,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let archive = client
                .archive_get_public(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get archive: {e}")))?;

            Ok(PyPublicArchive { inner: archive })
        })
    }

    /// Get the cost of creating a new vault.
    fn vault_cost<'a>(
        &self,
        py: Python<'a>,
        key: &PyVaultSecretKey,
        max_expected_size: u64,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let key = key.inner.clone();

        future_into_py(py, async move {
            let cost = client
                .vault_cost(&key, max_expected_size)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get vault cost: {e}")))?;
            Ok(cost.to_string())
        })
    }

    /// Put data into the client's VaultPacket
    ///
    /// Dynamically expand the vault capacity by paying for more space (Scratchpad) when needed.
    ///
    /// It is recommended to use the hash of the app name or unique identifier as the content type.
    fn write_bytes_to_vault<'a>(
        &self,
        py: Python<'a>,
        data: Vec<u8>,
        payment: &PyPaymentOption,
        key: &PyVaultSecretKey,
        content_type: u64,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment.inner.clone();
        let key = key.inner.clone();

        future_into_py(py, async move {
            match client
                .write_bytes_to_vault(bytes::Bytes::from(data), payment, &key, content_type)
                .await
            {
                Ok(cost) => Ok(cost.to_string()),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to write to vault: {e}"
                ))),
            }
        })
    }

    /// Get the register history, starting from the root to the latest entry.
    ///
    /// This returns a [`RegisterHistory`] that can be use to get the register values from the history.
    ///
    /// [`RegisterHistory::next`] can be used to get the values one by one, from the first to the latest entry.
    /// [`RegisterHistory::collect`] can be used to get all the register values from the history from the first to the latest entry.
    fn register_history(&self, addr: String) -> PyResult<PyRegisterHistory> {
        let client = self.inner.clone();
        let addr = RegisterAddress::from_hex(&addr)
            .map_err(|e| PyValueError::new_err(format!("Failed to parse address: {e}")))?;

        let history = client.register_history(&addr);
        Ok(PyRegisterHistory::new(history))
    }

    /// Create a new register key from a SecretKey and a name.
    ///
    /// This derives a new `SecretKey` from the owner's `SecretKey` using the name.
    /// Note that you will need to keep track of the names you used to create the register key.
    #[staticmethod]
    fn register_key_from_name(owner: PySecretKey, name: &str) -> PyResult<PySecretKey> {
        let key = Client::register_key_from_name(&owner.inner, name);
        Ok(PySecretKey { inner: key })
    }

    /// Create a new RegisterValue from bytes, make sure the bytes are not longer than `REGISTER_VALUE_SIZE`
    #[staticmethod]
    fn register_value_from_bytes(bytes: &[u8]) -> PyResult<[u8; 32]> {
        let value = Client::register_value_from_bytes(bytes)
            .map_err(|e| PyValueError::new_err(format!("`bytes` has invalid length: {e}")))?;
        Ok(value)
    }

    /// Create a new register with an initial value.
    ///
    /// Note that two payments are required, one for the underlying `GraphEntry` and one for the `Pointer`.
    fn register_create<'a>(
        &self,
        py: Python<'a>,
        owner: PySecretKey,
        value: [u8; 32],
        payment: PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .register_create(&owner.inner, value, payment.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to create register: {e}")))?;

            Ok((cost.to_string(), addr.to_hex()))
        })
    }

    /// Update the value of a register.
    ///
    /// The register needs to be created first with `register_create`.
    fn register_update<'a>(
        &self,
        py: Python<'a>,
        owner: PySecretKey,
        value: [u8; 32],
        payment: PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client
                .register_update(&owner.inner, value, payment.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to update register: {e}")))?;

            Ok(cost.to_string())
        })
    }

    /// Get the current value of the register
    fn register_get<'a>(&self, py: Python<'a>, addr: String) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let addr = RegisterAddress::from_hex(&addr)
            .map_err(|e| PyValueError::new_err(format!("Failed to parse address: {e}")))?;

        future_into_py(py, async move {
            let data = client
                .register_get(&addr)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get register: {e}")))?;

            Ok(data)
        })
    }

    /// Get the current value of the register
    fn register_cost<'a>(&self, py: Python<'a>, owner: PyPublicKey) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let cost = client.register_cost(&owner.inner).await.map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to get register cost: {e}"))
            })?;

            Ok(cost.to_string())
        })
    }

    /// Retrieves and returns a decrypted vault if one exists.
    ///
    /// Returns the content type of the bytes in the vault.
    fn fetch_and_decrypt_vault<'a>(
        &self,
        py: Python<'a>,
        key: &PyVaultSecretKey,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let key = key.inner.clone();

        future_into_py(py, async move {
            match client.fetch_and_decrypt_vault(&key).await {
                Ok((data, content_type)) => Ok((data.to_vec(), content_type)),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to fetch vault: {e}"
                ))),
            }
        })
    }

    /// Get the user data from the vault
    fn get_user_data_from_vault<'a>(
        &self,
        py: Python<'a>,
        key: &PyVaultSecretKey,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let key = key.inner.clone();

        future_into_py(py, async move {
            match client.get_user_data_from_vault(&key).await {
                Ok(user_data) => Ok(PyUserData { inner: user_data }),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to get user data from vault: {e}"
                ))),
            }
        })
    }

    /// Put the user data to the vault.
    ///
    /// Returns the total cost of the put operation.
    fn put_user_data_to_vault<'a>(
        &self,
        py: Python<'a>,
        key: &PyVaultSecretKey,
        payment: &PyPaymentOption,
        user_data: &PyUserData,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let key = key.inner.clone();
        let payment = payment.inner.clone();
        let user_data = user_data.inner.clone();

        future_into_py(py, async move {
            match client
                .put_user_data_to_vault(&key, payment, user_data)
                .await
            {
                Ok(cost) => Ok(cost.to_string()),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to put user data: {e}"
                ))),
            }
        })
    }

    /// Get a pointer from the network
    fn pointer_get<'a>(
        &self,
        py: Python<'a>,
        addr: PyPointerAddress,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            match client.pointer_get(&addr.inner).await {
                Ok(pointer) => Ok(PyPointer { inner: pointer }),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to get pointer: {e}"
                ))),
            }
        })
    }

    /// Check if a pointer exists on the network
    fn pointer_check_existance<'a>(
        &self,
        py: Python<'a>,
        addr: PyPointerAddress,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            let exists = client
                .pointer_check_existance(&addr.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get pointer: {e}")))?;

            Ok(exists)
        })
    }

    /// Manually store a pointer on the network
    fn pointer_put<'a>(
        &self,
        py: Python<'a>,
        pointer: &PyPointer,
        payment_option: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let pointer = pointer.inner.clone();
        let payment = payment_option.inner.clone();

        future_into_py(py, async move {
            let (_cost, addr) = client
                .pointer_put(pointer, payment)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to put pointer: {e}")))?;
            Ok(PyPointerAddress { inner: addr })
        })
    }

    /// Create a new pointer on the network.
    ///
    /// Make sure that the owner key is not already used for another pointer as each key is associated with one pointer
    fn pointer_create<'a>(
        &self,
        py: Python<'a>,
        owner: PySecretKey,
        target: PyPointerTarget,
        payment_option: &PyPaymentOption,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let payment = payment_option.inner.clone();

        future_into_py(py, async move {
            let (cost, addr) = client
                .pointer_create(&owner.inner, target.inner, payment)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to create pointer: {e}")))?;

            Ok((cost.to_string(), PyPointerAddress { inner: addr }))
        })
    }

    /// Update an existing pointer to point to a new target on the network.
    ///
    /// The pointer needs to be created first with `pointer_put`.
    /// This operation is free as the pointer was already paid for at creation.
    /// Only the latest version of the pointer is kept on the Network, previous versions will be overwritten and unrecoverable.
    fn pointer_update<'a>(
        &self,
        py: Python<'a>,
        owner: PySecretKey,
        target: PyPointerTarget,
    ) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();

        future_into_py(py, async move {
            client
                .pointer_update(&owner.inner, target.inner)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to update pointer: {e}")))?;

            Ok(())
        })
    }

    /// Calculate the cost of storing a pointer
    fn pointer_cost<'a>(&self, py: Python<'a>, key: &PyPublicKey) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        let key = key.inner;

        future_into_py(py, async move {
            match client.pointer_cost(&key).await {
                Ok(cost) => Ok(cost.to_string()),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to get pointer cost: {e}"
                ))),
            }
        })
    }
}

/// A network address where a pointer is stored.
/// The address is derived from the owner's public key.
#[pyclass(name = "PointerAddress")]
#[derive(Debug, Clone)]
pub struct PyPointerAddress {
    inner: PointerAddress,
}

#[pymethods]
impl PyPointerAddress {
    /// Initialise pointer address from hex string.
    #[staticmethod]
    pub fn from_hex(hex: String) -> PyResult<Self> {
        let bytes = hex::decode(hex)
            .map_err(|e| PyValueError::new_err(format!("`hex` not a valid hex string: {e}")))?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| PyValueError::new_err("`hex` invalid: must be 32 bytes"))?;

        Ok(Self {
            inner: PointerAddress::new(XorName(bytes)),
        })
    }

    /// Returns the hex string representation of the pointer address.
    #[getter]
    pub fn hex(&self) -> String {
        let bytes: [u8; 32] = self.inner.xorname().0;
        hex::encode(bytes)
    }
}

/// Pointer, a mutable address pointing to other data on the Network.
/// It is stored at the owner's public key and can only be updated by the owner.
#[pyclass(name = "Pointer")]
#[derive(Debug, Clone)]
pub struct PyPointer {
    inner: Pointer,
}

#[pymethods]
impl PyPointer {
    /// Create a new pointer, signing it with the provided secret key.
    /// This pointer would be stored on the network at the provided key's public key.
    /// There can only be one pointer at a time at the same address (one per key).
    #[new]
    pub fn new(key: &PySecretKey, counter: u32, target: &PyPointerTarget) -> PyResult<Self> {
        Ok(Self {
            inner: Pointer::new(&key.inner, counter, target.inner.clone()),
        })
    }

    /// Returns the network address where this pointer is stored.
    pub fn address(&self) -> PyPointerAddress {
        PyPointerAddress {
            inner: self.inner.address(),
        }
    }

    /// Returns the hex string representation of the pointer's target.
    #[getter]
    fn hex(&self) -> String {
        let bytes: [u8; 32] = self.inner.xorname().0;
        hex::encode(bytes)
    }

    /// Returns the target that this pointer points to.
    #[getter]
    fn target(&self) -> PyPointerTarget {
        PyPointerTarget {
            inner: PointerTarget::ChunkAddress(ChunkAddress::new(self.inner.xorname())),
        }
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.hex())
    }
}

/// The target that a pointer points to on the network.
#[pyclass(name = "PointerTarget")]
#[derive(Debug, Clone)]
pub struct PyPointerTarget {
    inner: PointerTarget,
}

#[pymethods]
impl PyPointerTarget {
    /// Initialize a pointer target from a chunk address hex string.
    #[staticmethod]
    fn from_hex(hex: &str) -> PyResult<Self> {
        let bytes = hex::decode(hex)
            .map_err(|e| PyValueError::new_err(format!("`hex` not a valid hex string: {e}")))?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| PyValueError::new_err("`hex` invalid: must be 32 bytes"))?;

        Ok(Self {
            inner: PointerTarget::ChunkAddress(ChunkAddress::new(XorName(bytes))),
        })
    }

    /// Returns the hex string representation of this pointer address.
    #[getter]
    fn hex(&self) -> String {
        let bytes: [u8; 32] = self.inner.xorname().0;
        hex::encode(bytes)
    }

    #[getter]
    fn target(&self) -> PyPointerTarget {
        PyPointerTarget {
            inner: PointerTarget::ChunkAddress(ChunkAddress::new(self.inner.xorname())),
        }
    }

    /// Creates a pointer target from a chunk address.
    #[staticmethod]
    fn from_chunk_address(addr: &PyChunkAddress) -> Self {
        Self {
            inner: PointerTarget::ChunkAddress(addr.inner),
        }
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.hex())
    }
}

/// An address of a chunk of data on the network. Used to locate and retrieve data chunks.
#[pyclass(name = "ChunkAddress")]
#[derive(Debug, Clone)]
pub struct PyChunkAddress {
    inner: ChunkAddress,
}

impl From<ChunkAddress> for PyChunkAddress {
    fn from(addr: ChunkAddress) -> Self {
        Self { inner: addr }
    }
}

impl From<PyChunkAddress> for ChunkAddress {
    fn from(addr: PyChunkAddress) -> Self {
        addr.inner
    }
}

#[pymethods]
impl PyChunkAddress {
    /// Creates a new chunk address from a string representation.
    #[new]
    fn new(#[pyo3(from_py_with = "str_to_addr")] addr: XorName) -> PyResult<Self> {
        Ok(Self {
            inner: ChunkAddress::new(addr),
        })
    }

    #[getter]
    fn hex(&self) -> String {
        let bytes: [u8; 32] = self.inner.xorname().0;
        hex::encode(bytes)
    }

    /// Creates a chunk address from a hex string representation.
    #[staticmethod]
    fn from_chunk_address(addr: &str) -> PyResult<Self> {
        let bytes =
            hex::decode(addr).map_err(|e| PyValueError::new_err(format!("`addr` invalid: {e}")))?;

        if bytes.len() != 32 {
            return Err(PyValueError::new_err("`addr` invalid: must be 32 bytes"));
        }

        let mut xorname = [0u8; 32];
        xorname.copy_from_slice(&bytes);

        Ok(Self {
            inner: ChunkAddress::new(XorName(xorname)),
        })
    }

    fn __str__(&self) -> PyResult<String> {
        Ok(self.hex())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ChunkAddress({})", self.hex()))
    }
}

/// A wallet for interacting with the network's payment system.
/// Handles token transfers, balance checks, and payments for network operations.
#[pyclass(name = "Wallet")]
#[derive(Clone)]
pub struct PyWallet {
    pub(crate) inner: Wallet,
}

#[pymethods]
impl PyWallet {
    /// Creates a new wallet from a private key string.
    /// The wallet will be configured to use the ArbitrumOne network.
    #[new]
    fn new(private_key: String) -> PyResult<Self> {
        let wallet = Wallet::new_from_private_key(
            Network::ArbitrumOne, // TODO: Make this configurable
            &private_key,
        )
        .map_err(|e| PyValueError::new_err(format!("`private_key` invalid: {e}")))?;

        Ok(Self { inner: wallet })
    }

    /// Creates a new wallet from a private key string with a specified network.
    #[staticmethod]
    fn new_from_private_key(network: PyNetwork, private_key: &str) -> PyResult<Self> {
        let inner = Wallet::new_from_private_key(network.inner, private_key)
            .map_err(|e| PyValueError::new_err(format!("`private_key` invalid: {e}")))?;

        Ok(Self { inner })
    }

    /// Returns a string representation of the wallet's address.
    fn address(&self) -> String {
        format!("{:?}", self.inner.address())
    }

    /// Returns the raw balance of payment tokens in the wallet.
    fn balance<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        future_into_py(py, async move {
            match client.balance_of_tokens().await {
                Ok(balance) => Ok(balance.to_string()),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to get balance: {e}"
                ))),
            }
        })
    }

    /// Returns the current balance of gas tokens in the wallet.
    fn balance_of_gas<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let client = self.inner.clone();
        future_into_py(py, async move {
            match client.balance_of_gas_tokens().await {
                Ok(balance) => Ok(balance.to_string()),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to get balance: {e}"
                ))),
            }
        })
    }
}

/// Options for making payments on the network.
#[pyclass(name = "PaymentOption")]
#[derive(Clone)]
pub struct PyPaymentOption {
    pub(crate) inner: PaymentOption,
}

#[pymethods]
impl PyPaymentOption {
    /// Creates a payment option using the provided wallet.
    #[staticmethod]
    fn wallet(wallet: &PyWallet) -> Self {
        Self {
            inner: PaymentOption::Wallet(wallet.inner.clone()),
        }
    }
}

/// A cryptographic secret key used for signing operations.
/// Can be used to derive a public key and perform cryptographic operations.
#[pyclass(name = "SecretKey")]
#[derive(Debug, Clone)]
pub struct PySecretKey {
    inner: SecretKey,
}

#[pymethods]
impl PySecretKey {
    /// Creates a new random secret key.
    #[new]
    fn new() -> PyResult<Self> {
        Ok(Self {
            inner: SecretKey::random(),
        })
    }

    /// Creates a secret key from a hex string representation.
    #[staticmethod]
    fn from_hex(hex_str: &str) -> PyResult<Self> {
        SecretKey::from_hex(hex_str)
            .map(|key| Self { inner: key })
            .map_err(|e| PyValueError::new_err(format!("Invalid hex key: {e}")))
    }

    /// Derives and returns the corresponding public key.
    fn public_key(&self) -> PyPublicKey {
        PyPublicKey {
            inner: self.inner.public_key(),
        }
    }

    /// Returns the hex string representation of the key.
    fn to_hex(&self) -> String {
        self.inner.to_hex()
    }
}

/// A cryptographic public key derived from a secret key.
#[pyclass(name = "PublicKey")]
#[derive(Debug, Clone)]
pub struct PyPublicKey {
    inner: PublicKey,
}

#[pymethods]
impl PyPublicKey {
    /// Creates a new random public key by generating a random secret key.
    #[new]
    fn new() -> PyResult<Self> {
        let secret = SecretKey::random();
        Ok(Self {
            inner: secret.public_key(),
        })
    }

    /// Creates a public key from a hex string representation.
    #[staticmethod]
    fn from_hex(hex_str: &str) -> PyResult<Self> {
        PublicKey::from_hex(hex_str)
            .map(|key| Self { inner: key })
            .map_err(|e| PyValueError::new_err(format!("Invalid hex key: {e}")))
    }

    fn to_hex(&self) -> String {
        self.inner.to_hex()
    }
}

/// A secret key used to encrypt and decrypt vault data.
#[pyclass(name = "VaultSecretKey")]
#[derive(Debug, Clone)]
pub struct PyVaultSecretKey {
    inner: VaultSecretKey,
}

#[pymethods]
impl PyVaultSecretKey {
    /// Creates a new random vault secret key.
    #[new]
    fn new() -> PyResult<Self> {
        Ok(Self {
            inner: VaultSecretKey::random(),
        })
    }

    #[staticmethod]
    fn from_hex(hex_str: &str) -> PyResult<Self> {
        VaultSecretKey::from_hex(hex_str)
            .map(|key| Self { inner: key })
            .map_err(|e| PyValueError::new_err(format!("Invalid hex key: {e}")))
    }

    fn to_hex(&self) -> String {
        self.inner.to_hex()
    }
}

/// UserData is stored in Vaults and contains most of a user's private data:
/// It allows users to keep track of only the key to their User Data Vault
/// while having the rest kept on the Network encrypted in a Vault for them
/// Using User Data Vault is optional, one can decide to keep all their data locally instead.
#[pyclass(name = "UserData")]
#[derive(Debug, Clone)]
pub struct PyUserData {
    inner: UserData,
}

#[pymethods]
impl PyUserData {
    /// Creates a new empty UserData instance.
    #[new]
    fn new() -> Self {
        Self {
            inner: UserData::new(),
        }
    }

    /// Returns a list of public file archives as (address, name) pairs.
    fn file_archives(&self) -> Vec<(String, String)> {
        self.inner
            .file_archives
            .iter()
            .map(|(addr, name)| (hex::encode(addr), name.clone()))
            .collect()
    }

    /// Returns a list of private file archives as (data_map, name) pairs.
    fn private_file_archives(&self) -> Vec<(String, String)> {
        self.inner
            .private_file_archives
            .iter()
            .map(|(addr, name)| (addr.to_hex(), name.clone()))
            .collect()
    }
}

/// A map with encrypted data pieces on the network. Used to locate and reconstruct private data.
#[pyclass(name = "DataMapChunk")]
#[derive(Debug, Clone)]
pub struct PyDataMapChunk {
    inner: DataMapChunk,
}

#[pymethods]
impl PyDataMapChunk {
    /// Creates a DataMapChunk from a hex string representation.
    #[staticmethod]
    fn from_hex(hex: &str) -> PyResult<Self> {
        DataMapChunk::from_hex(hex)
            .map(|access| Self { inner: access })
            .map_err(|e| PyValueError::new_err(format!("Invalid hex: {e}")))
    }

    /// Returns the hex string representation of this DataMapChunk.
    fn to_hex(&self) -> String {
        self.inner.to_hex()
    }

    /// Returns the private address of this DataMapChunk.
    ///
    /// Note that this is not a network address, it is only used for refering to private data client side.
    fn address(&self) -> String {
        self.inner.address().to_string()
    }
}

#[pyfunction]
fn encrypt(data: Vec<u8>) -> PyResult<(Vec<u8>, Vec<Vec<u8>>)> {
    let (data_map, chunks) = self_encryption::encrypt(Bytes::from(data))
        .map_err(|e| PyRuntimeError::new_err(format!("Encryption failed: {e}")))?;

    let data_map_bytes = rmp_serde::to_vec(&data_map)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize data map: {e}")))?;

    let chunks_bytes: Vec<Vec<u8>> = chunks
        .into_iter()
        .map(|chunk| chunk.content.to_vec())
        .collect();

    Ok((data_map_bytes, chunks_bytes))
}

#[pyclass(name = "Network")]
#[derive(Debug, Clone)]
pub struct PyNetwork {
    inner: Network,
}

#[pymethods]
impl PyNetwork {
    /// Creates a new network configuration.
    ///
    /// If `local` is true, configures for local network connections.
    #[new]
    fn new(local: bool) -> PyResult<Self> {
        let inner = Network::new(local).map_err(|e| PyRuntimeError::new_err(format!("{e:?}")))?;
        Ok(Self { inner })
    }
}

/// Metadata for files in an archive, containing creation time, modification time, and size.
#[pyclass(name = "Metadata")]
#[derive(Debug, Clone)]
pub struct PyMetadata {
    inner: Metadata,
}

#[pymethods]
impl PyMetadata {
    /// Create new metadata with the given file size
    #[new]
    fn new(size: u64) -> Self {
        Self {
            inner: Metadata::new_with_size(size),
        }
    }

    /// Get the creation time as Unix timestamp in seconds
    #[getter]
    fn get_created(&self) -> u64 {
        self.inner.created
    }

    /// Set the creation time as Unix timestamp in seconds
    #[setter]
    fn set_created(&mut self, value: u64) {
        self.inner.created = value;
    }

    /// Get the modification time as Unix timestamp in seconds
    #[getter]
    fn get_modified(&self) -> u64 {
        self.inner.modified
    }

    /// Set the modification time as Unix timestamp in seconds
    #[setter]
    fn set_modified(&mut self, value: u64) {
        self.inner.modified = value;
    }

    /// Get the file size in bytes
    #[getter]
    fn get_size(&self) -> u64 {
        self.inner.size
    }

    /// Set the file size in bytes
    #[setter]
    fn set_size(&mut self, value: u64) {
        self.inner.size = value;
    }
}

/// A public archive containing files that can be accessed by anyone on the network.
#[pyclass(name = "PublicArchive")]
#[derive(Debug, Clone)]
pub struct PyPublicArchive {
    inner: PublicArchive,
}

#[pymethods]
impl PyPublicArchive {
    /// Create a new empty archive
    #[new]
    fn new() -> Self {
        Self {
            inner: PublicArchive::new(),
        }
    }

    /// Rename a file in the archive.
    ///
    /// Returns None on success, or error message on failure
    fn rename_file(&mut self, old_path: PathBuf, new_path: PathBuf) -> PyResult<()> {
        self.inner
            .rename_file(&old_path, &new_path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to rename file: {e}")))
    }

    /// Add a file to the archive
    fn add_file(
        &mut self,
        path: PathBuf,
        #[pyo3(from_py_with = "str_to_addr")] addr: XorName,
        metadata: &PyMetadata,
    ) {
        self.inner.add_file(path, addr, metadata.inner.clone());
    }

    /// List all files in the archive.
    ///
    /// Returns a list of (path, metadata) tuples
    fn files(&self) -> Vec<(PathBuf, PyMetadata)> {
        self.inner
            .files()
            .into_iter()
            .map(|(path, meta)| (path, PyMetadata { inner: meta }))
            .collect()
    }

    /// List all data addresses of files in the archive
    fn addresses(&self) -> Vec<String> {
        self.inner
            .addresses()
            .into_iter()
            .map(crate::client::address::addr_to_str)
            .collect()
    }
}

/// A public archive containing files that can be accessed by anyone on the network.
#[pyclass(name = "PrivateArchive")]
#[derive(Debug, Clone)]
pub struct PyPrivateArchive {
    inner: PrivateArchive,
}

#[pymethods]
impl PyPrivateArchive {
    /// Create a new empty archive
    #[new]
    fn new() -> Self {
        Self {
            inner: PrivateArchive::new(),
        }
    }

    /// Rename a file in the archive.
    ///
    /// Returns None on success, or error message on failure
    fn rename_file(&mut self, old_path: PathBuf, new_path: PathBuf) -> PyResult<()> {
        self.inner
            .rename_file(&old_path, &new_path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to rename file: {e}")))
    }

    /// Add a file to a local archive. Note that this does not upload the archive to the network.
    fn add_file(&mut self, path: PathBuf, data_map: &PyDataMapChunk, metadata: &PyMetadata) {
        self.inner
            .add_file(path, data_map.inner.clone(), metadata.inner.clone());
    }

    /// List all files in the archive.
    fn files(&self) -> Vec<(PathBuf, PyMetadata)> {
        self.inner
            .files()
            .into_iter()
            .map(|(path, meta)| (path, PyMetadata { inner: meta }))
            .collect()
    }

    /// List all data maps of files in the archive
    fn data_maps(&self) -> Vec<PyDataMapChunk> {
        self.inner
            .data_maps()
            .into_iter()
            .map(|data_map| PyDataMapChunk { inner: data_map })
            .collect()
    }
}

/// A generic GraphEntry on the Network.
///
/// Graph entries are stored at the owner's public key. Note that there can only be one graph entry per owner.
/// Graph entries can be linked to other graph entries as parents or descendants.
/// Applications are free to define the meaning of these links, those are not enforced by the protocol.
/// The protocol only ensures that the graph entry is immutable once uploaded and that the signature is valid and matches the owner.
///
/// For convenience it is advised to make use of BLS key derivation to create multiple graph entries from a single key.
#[pyclass(name = "GraphEntry")]
#[derive(Debug, Clone)]
pub struct PyGraphEntry {
    inner: GraphEntry,
}

/// Scratchpad, a mutable space for encrypted data on the Network
#[pyclass(name = "Scratchpad")]
#[derive(Debug, Clone)]
pub struct PyScratchpad {
    inner: Scratchpad,
}

/// A handle to the register history
#[pyclass(name = "RegisterHistory")]
#[derive(Clone)]
pub struct PyRegisterHistory {
    inner: Arc<futures::lock::Mutex<RegisterHistory>>,
}

impl PyRegisterHistory {
    fn new(history: RegisterHistory) -> Self {
        Self {
            inner: Arc::new(futures::lock::Mutex::new(history)),
        }
    }
}

#[pymethods]
impl PyRegisterHistory {
    fn next<'a>(&'a mut self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let arc = Arc::clone(&self.inner);

        future_into_py(py, async move {
            let mut register_history = arc.lock().await;
            let value = register_history
                .next()
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("history `next` failed: {e}")))?;

            Ok(value)
        })
    }

    fn collect<'a>(&'a mut self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let arc = Arc::clone(&self.inner);

        future_into_py(py, async move {
            let mut register_history = arc.lock().await;
            let values = register_history
                .collect()
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("history `collect` failed: {e}")))?;

            Ok(values)
        })
    }
}

/// Configuration for the `Client` which can be provided through: `init_with_config`.
#[pyclass(name = "ClientConfig")]
#[derive(Debug, Clone)]
pub struct PyClientConfig {
    inner: ClientConfig,
}

#[pymethods]
impl PyClientConfig {
    #[staticmethod]
    fn new() -> Self {
        Self {
            inner: ClientConfig::default(),
        }
    }

    /// Whether we're expected to connect to a local network.
    #[getter]
    fn get_local(&self) -> bool {
        self.inner.local
    }

    /// Whether we're expected to connect to a local network.
    #[setter]
    fn set_local(&mut self, value: bool) {
        self.inner.local = value;
    }

    /// List of peers to connect to.
    ///
    /// If not provided, the client will use the default bootstrap peers.
    #[getter]
    fn get_peers(&self) -> Option<Vec<String>> {
        self.inner
            .peers
            .as_ref()
            .map(|peers| peers.iter().map(|p| p.to_string()).collect())
    }

    /// List of peers to connect to. If given empty list, the client will use the default bootstrap peers.
    #[setter]
    fn set_peers(&mut self, peers: Vec<String>) -> PyResult<()> {
        if peers.is_empty() {
            self.inner.peers = None;
            return Ok(());
        }

        let peers: Vec<Multiaddr> = peers
            .iter()
            .map(|p| Multiaddr::from_str(p))
            .collect::<Result<_, _>>()
            .map_err(|e| PyValueError::new_err(format!("Failed to parse peers: {e}")))?;

        self.inner.peers = Some(peers);
        Ok(())
    }

    /// EVM network to use for quotations and payments.
    #[getter]
    fn get_network(&self) -> PyNetwork {
        PyNetwork {
            inner: self.inner.evm_network.clone(),
        }
    }

    /// EVM network to use for quotations and payments.
    #[setter]
    fn set_network(&mut self, network: PyNetwork) {
        self.inner.evm_network = network.inner;
    }

    // TODO
    // fn strategy() { }
}

#[pymodule]
#[pyo3(name = "autonomi_client")]
fn autonomi_client_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyClient>()?;
    m.add_class::<PyWallet>()?;
    m.add_class::<PyPaymentOption>()?;
    m.add_class::<PyVaultSecretKey>()?;
    m.add_class::<PyUserData>()?;
    m.add_class::<PyDataMapChunk>()?;
    m.add_class::<PyPointer>()?;
    m.add_class::<PyPointerAddress>()?;
    m.add_class::<PyPointerTarget>()?;
    m.add_class::<PyChunkAddress>()?;
    m.add_class::<PySecretKey>()?;
    m.add_class::<PyPublicKey>()?;
    m.add_class::<PyNetwork>()?;
    m.add_class::<PyMetadata>()?;
    m.add_class::<PyPublicArchive>()?;
    m.add_class::<PyPrivateArchive>()?;
    m.add_class::<PyGraphEntry>()?;
    m.add_class::<PyScratchpad>()?;
    m.add_class::<PyRegisterHistory>()?;
    m.add_class::<PyClientConfig>()?;
    m.add_function(wrap_pyfunction!(encrypt, m)?)?;
    Ok(())
}

// Helper function to convert argument hex string to XorName.
fn str_to_addr(addr: &Bound<'_, PyAny>) -> PyResult<XorName> {
    let addr: String = addr.extract()?;
    crate::client::address::str_to_addr(&addr)
        .map_err(|e| PyValueError::new_err(format!("`addr` has invalid format: {e:?}")))
}

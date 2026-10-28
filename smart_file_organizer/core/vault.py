"""
Cryptographic Encrypted Vault (Zero-Knowledge AES-256-GCM / PBKDF2 Container).
Provides authenticated, tamper-evident local file and folder locking into a portable '.sfovault' container.
Uses 100,000 rounds of PBKDF2-HMAC-SHA256 key derivation with 256-bit AES-GCM encryption.
Supports optional automatic secure shredding of source files upon successful vault locking.
Zero external runtime dependencies (with automatic high-speed AES acceleration).
"""

import io
import os
import sys
import time
import hmac
import zipfile
import hashlib
from pathlib import Path
from dataclasses import dataclass
from typing import List, Dict, Any, Optional, Tuple, Union

try:
    from Crypto.Cipher import AES
    _HAS_PYCRYPTODOME = True
except ImportError:
    _HAS_PYCRYPTODOME = False

from smart_file_organizer.core.enterprise_guardrails import EnterpriseGuardrails, AuditLogChain
from smart_file_organizer.core.shredder import FileShredder, ShredMethod


class VaultError(Exception):
    """Base exception for Vault operations."""
    pass


class InvalidPasswordError(VaultError):
    """Raised when an incorrect password or recovery key is provided."""
    pass


class CorruptedVaultError(VaultError):
    """Raised when vault headers or ciphertext fail authentication integrity checks."""
    pass


class VaultProtectionError(VaultError):
    """Raised when an operation targets protected system paths."""
    pass


@dataclass
class VaultMetadata:
    """Summary information about a vault container."""
    vault_path: Path
    file_count: int
    total_uncompressed_bytes: int
    created_version: int
    files: List[Dict[str, Any]]


class VaultEngine:
    """
    Core cryptographic engine supporting AES-256-GCM with standard library fallback.
    """
    MAGIC = b"SFOVAULT"
    VERSION = 1
    SALT_SIZE = 32
    NONCE_SIZE = 12
    TAG_SIZE = 16
    PBKDF2_ITERATIONS = 100_000

    @classmethod
    def derive_key(cls, password: str, salt: bytes, dklen: int = 32) -> bytes:
        """Derives a 256-bit cryptographic key using PBKDF2-HMAC-SHA256."""
        return hashlib.pbkdf2_hmac(
            "sha256",
            password.encode("utf-8"),
            salt,
            iterations=cls.PBKDF2_ITERATIONS,
            dklen=dklen
        )

    @classmethod
    def encrypt_data(cls, plaintext: bytes, password: str) -> bytes:
        """
        Encrypts plaintext bytes with password.
        Returns: MAGIC + VERSION(2B) + SALT(32B) + NONCE(12B) + TAG(16B) + CIPHERTEXT
        """
        salt = os.urandom(cls.SALT_SIZE)
        nonce = os.urandom(cls.NONCE_SIZE)
        version_bytes = cls.VERSION.to_bytes(2, byteorder="big")

        if _HAS_PYCRYPTODOME:
            key = cls.derive_key(password, salt, dklen=32)
            cipher = AES.new(key, AES.MODE_GCM, nonce=nonce)
            # Authenticate header as Additional Authenticated Data (AAD)
            aad = cls.MAGIC + version_bytes + salt + nonce
            cipher.update(aad)
            ciphertext, auth_tag = cipher.encrypt_and_digest(plaintext)
        else:
            # High-fidelity authenticated stream cipher fallback using SHA-256 CTR + HMAC
            keys = cls.derive_key(password, salt, dklen=64)
            enc_key = keys[:32]
            mac_key = keys[32:]
            ciphertext = cls._xor_stream(plaintext, enc_key, nonce)
            aad = cls.MAGIC + version_bytes + salt + nonce
            auth_tag = hmac.new(mac_key, aad + ciphertext, hashlib.sha256).digest()[:cls.TAG_SIZE]

        return cls.MAGIC + version_bytes + salt + nonce + auth_tag + ciphertext

    @classmethod
    def decrypt_data(cls, vault_bytes: bytes, password: str) -> bytes:
        """
        Authenticates and decrypts vault payload bytes.
        Raises InvalidPasswordError or CorruptedVaultError on failure.
        """
        min_header_size = len(cls.MAGIC) + 2 + cls.SALT_SIZE + cls.NONCE_SIZE + cls.TAG_SIZE
        if len(vault_bytes) < min_header_size:
            raise CorruptedVaultError("Vault file is truncated or invalid.")

        offset = 0
        magic = vault_bytes[offset:offset + len(cls.MAGIC)]
        offset += len(cls.MAGIC)
        if magic != cls.MAGIC:
            raise CorruptedVaultError("Invalid vault header: not an SFO Vault container.")

        version = int.from_bytes(vault_bytes[offset:offset + 2], byteorder="big")
        offset += 2
        salt = vault_bytes[offset:offset + cls.SALT_SIZE]
        offset += cls.SALT_SIZE
        nonce = vault_bytes[offset:offset + cls.NONCE_SIZE]
        offset += cls.NONCE_SIZE
        auth_tag = vault_bytes[offset:offset + cls.TAG_SIZE]
        offset += cls.TAG_SIZE
        ciphertext = vault_bytes[offset:]

        aad = cls.MAGIC + version.to_bytes(2, byteorder="big") + salt + nonce

        if _HAS_PYCRYPTODOME:
            key = cls.derive_key(password, salt, dklen=32)
            cipher = AES.new(key, AES.MODE_GCM, nonce=nonce)
            cipher.update(aad)
            try:
                plaintext = cipher.decrypt_and_verify(ciphertext, auth_tag)
                return plaintext
            except ValueError:
                raise InvalidPasswordError("Incorrect password or tampered vault data.")
        else:
            keys = cls.derive_key(password, salt, dklen=64)
            enc_key = keys[:32]
            mac_key = keys[32:]
            expected_tag = hmac.new(mac_key, aad + ciphertext, hashlib.sha256).digest()[:cls.TAG_SIZE]
            if not hmac.compare_digest(auth_tag, expected_tag):
                raise InvalidPasswordError("Incorrect password or tampered vault data.")
            return cls._xor_stream(ciphertext, enc_key, nonce)

    @classmethod
    def _xor_stream(cls, data: bytes, key: bytes, nonce: bytes) -> bytes:
        """Standard library stream encryption/decryption using counter keystream."""
        output = bytearray(len(data))
        block_size = 32
        counter = 0
        data_len = len(data)

        for i in range(0, data_len, block_size):
            block_counter = counter.to_bytes(8, byteorder="big")
            keystream = hashlib.sha256(key + nonce + block_counter).digest()
            chunk_len = min(block_size, data_len - i)
            for j in range(chunk_len):
                output[i + j] = data[i + j] ^ keystream[j]
            counter += 1

        return bytes(output)


class VaultManager:
    """
    High-level manager for locking and unlocking files/directories into .sfovault containers.
    """

    @classmethod
    def lock_path(
        cls,
        source_path: Union[str, Path],
        vault_path: Union[str, Path],
        password: str,
        shred_source: bool = False,
        audit_chain: Optional[AuditLogChain] = None
    ) -> Path:
        """
        Archives source path into an encrypted .sfovault container.
        
        Args:
            source_path: File or directory to encrypt and lock.
            vault_path: Destination path for .sfovault file.
            password: Passphrase for AES-256-GCM encryption.
            shred_source: If True, executes military-grade DoD shredding of source files.
            audit_chain: Optional audit log chain.
            
        Returns:
            Resolved Path of the created vault container.
        """
        src = Path(source_path).resolve()
        dst = Path(vault_path).resolve()

        if not src.exists():
            raise FileNotFoundError(f"Source path not found: {src}")

        # Guardrails verification
        is_protected, reason = EnterpriseGuardrails.is_protected_path(src)
        if is_protected:
            raise VaultProtectionError(f"Action blocked by Enterprise Guardrails: {reason}")

        if not dst.name.endswith(".sfovault"):
            dst = dst.with_suffix(".sfovault")

        dst.parent.mkdir(parents=True, exist_ok=True)

        # 1. Compress source into in-memory ZIP archive
        zip_buffer = io.BytesIO()
        with zipfile.ZipFile(zip_buffer, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
            if src.is_file():
                zf.write(src, arcname=src.name)
            else:
                for root, _, files in os.walk(src):
                    for file in files:
                        abs_file = Path(root) / file
                        rel_arc = abs_file.relative_to(src)
                        zf.write(abs_file, arcname=str(rel_arc))

        uncompressed_payload = zip_buffer.getvalue()

        # 2. Encrypt payload
        encrypted_vault_bytes = VaultEngine.encrypt_data(uncompressed_payload, password)

        # 3. Write vault file atomically via temporary file
        tmp_dst = dst.with_name(f".{dst.name}.tmp")
        with open(tmp_dst, "wb") as f:
            f.write(encrypted_vault_bytes)
            f.flush()
            try:
                os.fsync(f.fileno())
            except OSError:
                pass
        tmp_dst.replace(dst)

        # 4. Optional source shredding
        if shred_source:
            if src.is_file():
                FileShredder.shred_file(src, method=ShredMethod.DOD_3PASS, audit_chain=audit_chain)
            else:
                FileShredder.shred_directory(src, method=ShredMethod.DOD_3PASS, audit_chain=audit_chain)

        # 5. Audit Logging
        if audit_chain:
            audit_chain.log_event(
                action="VAULT_LOCK",
                details={
                    "source": str(src),
                    "vault": str(dst),
                    "shred_source": shred_source,
                    "vault_size_bytes": dst.stat().st_size
                }
            )

        return dst

    @classmethod
    def unlock_vault(
        cls,
        vault_path: Union[str, Path],
        dest_dir: Union[str, Path],
        password: str,
        audit_chain: Optional[AuditLogChain] = None
    ) -> List[Path]:
        """
        Decrypts an .sfovault container and extracts files to destination folder.
        
        Args:
            vault_path: Path to .sfovault file.
            dest_dir: Target directory to extract files into.
            password: Decryption passphrase.
            audit_chain: Optional audit log chain.
            
        Returns:
            List of extracted file Paths.
        """
        vault_file = Path(vault_path).resolve()
        target = Path(dest_dir).resolve()

        if not vault_file.exists():
            raise FileNotFoundError(f"Vault container not found: {vault_file}")

        target.mkdir(parents=True, exist_ok=True)

        with open(vault_file, "rb") as f:
            vault_bytes = f.read()

        # Decrypt payload
        plaintext_zip = VaultEngine.decrypt_data(vault_bytes, password)

        extracted_files: List[Path] = []
        with zipfile.ZipFile(io.BytesIO(plaintext_zip), "r") as zf:
            for member in zf.infolist():
                extracted_path = zf.extract(member, target)
                extracted_files.append(Path(extracted_path))

        # Audit Logging
        if audit_chain:
            audit_chain.log_event(
                action="VAULT_UNLOCK",
                details={
                    "vault": str(vault_file),
                    "dest_dir": str(target),
                    "extracted_count": len(extracted_files)
                }
            )

        return extracted_files

    @classmethod
    def inspect_vault(cls, vault_path: Union[str, Path], password: str) -> VaultMetadata:
        """
        Inspects the contents of a vault without extracting files to disk.
        """
        vault_file = Path(vault_path).resolve()
        if not vault_file.exists():
            raise FileNotFoundError(f"Vault container not found: {vault_file}")

        with open(vault_file, "rb") as f:
            vault_bytes = f.read()

        plaintext_zip = VaultEngine.decrypt_data(vault_bytes, password)

        files_info = []
        total_uncompressed = 0

        with zipfile.ZipFile(io.BytesIO(plaintext_zip), "r") as zf:
            for info in zf.infolist():
                if not info.is_dir():
                    files_info.append({
                        "filename": info.filename,
                        "file_size": info.file_size,
                        "date_time": f"{info.date_time[0]}-{info.date_time[1]:02d}-{info.date_time[2]:02d}"
                    })
                    total_uncompressed += info.file_size

        return VaultMetadata(
            vault_path=vault_file,
            file_count=len(files_info),
            total_uncompressed_bytes=total_uncompressed,
            created_version=VaultEngine.VERSION,
            files=files_info
        )

    @classmethod
    def verify_password(cls, vault_path: Union[str, Path], password: str) -> bool:
        """Returns True if password unlocks the vault, False otherwise."""
        try:
            cls.inspect_vault(vault_path, password)
            return True
        except (InvalidPasswordError, CorruptedVaultError):
            return False

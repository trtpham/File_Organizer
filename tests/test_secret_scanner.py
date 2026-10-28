"""
Unit tests for SecretScanner (DLP).
"""

from pathlib import Path
from smart_file_organizer.core.secret_scanner import SecretScanner


def test_detect_aws_and_openai_secrets(temp_workspace):
    env_file = temp_workspace / ".env"
    env_file.write_text(
        "AWS_KEY=AKIA1234567890ABCDEF\n"
        "OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz1234567890\n"
        "NORMAL_VAR=Hello\n"
    )

    findings = SecretScanner.scan_file(env_file)
    assert len(findings) == 2

    types = {f.secret_type for f in findings}
    assert "AWS Access Key" in types
    assert "OpenAI API Key" in types

    for f in findings:
        assert f.severity == "CRITICAL"
        assert "***" in f.masked_snippet


def test_quarantine_leaked_file(temp_workspace):
    key_file = temp_workspace / "id_rsa"
    key_file.write_text(
        "-----BEGIN OPENSSH PRIVATE KEY-----\n"
        "b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAACFwAAAAdzc2gtcn\n"
        "-----END OPENSSH PRIVATE KEY-----\n"
    )

    findings = SecretScanner.scan_directory(temp_workspace)
    assert len(findings) >= 1

    quarantine_res = SecretScanner.quarantine_files(findings, temp_workspace)
    assert quarantine_res["success"] is True
    assert quarantine_res["quarantined_count"] == 1

    # Original file is moved
    assert not key_file.exists()
    assert (temp_workspace / "_Quarantine" / "id_rsa.quarantined").exists()


def test_shannon_entropy_and_extended_tokens(temp_workspace):
    # Test entropy function
    low_entropy = SecretScanner.calculate_shannon_entropy("aaaaaaaaaa")
    high_entropy = SecretScanner.calculate_shannon_entropy("a8!Zq9#vLm2$xP0@")
    assert high_entropy > low_entropy
    assert low_entropy == 0.0

    # Test Slack and Stripe token detection (dynamically assembled to avoid git push false positives)
    mock_slack = "xoxb" + "-123456789012-123456789012-" + "abcdefghijklmnopqrstuvwx"
    mock_stripe = "sk_" + "live_1234567890abcdefghijklmnopqrstuv"
    token_file = temp_workspace / "tokens.txt"
    token_file.write_text(
        f"SLACK={mock_slack}\n"
        f"STRIPE={mock_stripe}\n"
    )

    findings = SecretScanner.scan_file(token_file)
    assert len(findings) == 2
    types = {f.secret_type for f in findings}
    assert "Slack Token" in types
    assert "Stripe Secret Key" in types

"""Tests for output normalization (thinking block stripping)."""

import pytest

from osai_model_router.providers import strip_thinking_blocks


def test_complete_thinking_block_is_stripped():
    """Test that complete <think>...</think> blocks are removed."""
    content = "<think> This is a thought process.</think> This is the answer."
    result = strip_thinking_blocks(content)

    assert result.content == "This is the answer."
    assert result.reasoning_stripped is True
    assert result.was_empty is False


def test_incomplete_thinking_block_is_stripped():
    """Test that incomplete <think> blocks on their own line are removed."""
    # Incomplete block on its own line, followed by actual content on next line
    content = "<think> This is an incomplete thought\nThis is the answer."
    result = strip_thinking_blocks(content)

    assert result.content == "This is the answer."
    assert result.reasoning_stripped is True
    assert result.was_empty is False


def test_normal_content_preserved():
    """Test that normal content outside thinking blocks is preserved."""
    content = "Hello, how are you?"
    result = strip_thinking_blocks(content)

    assert result.content == "Hello, how are you?"
    assert result.reasoning_stripped is False
    assert result.was_empty is False


def test_only_thinking_block_returns_fallback():
    """Test that stripping all content returns fallback message."""
    content = "<think> This is only reasoning.</think>"
    result = strip_thinking_blocks(content)

    assert result.content == "The model response contained only hidden reasoning and no visible answer."
    assert result.reasoning_stripped is True
    assert result.was_empty is True


def test_empty_content_returns_fallback():
    """Test that empty content returns fallback message."""
    content = ""
    result = strip_thinking_blocks(content)

    assert result.content == "The model response contained only hidden reasoning and no visible answer."
    assert result.was_empty is True


def test_whitespace_is_trimmed():
    """Test that whitespace is trimmed after stripping."""
    content = "<think> thought </think>    Answer here    "
    result = strip_thinking_blocks(content)

    assert result.content == "Answer here"
    assert result.reasoning_stripped is True


def test_multiple_thinking_blocks_stripped():
    """Test that multiple thinking blocks are all removed."""
    content = "<think> first </think> Middle <think> second </think> End"
    result = strip_thinking_blocks(content)

    assert result.content == "Middle  End"
    assert result.reasoning_stripped is True


def test_nested_content_preserved():
    """Test that content around thinking blocks is preserved."""
    content = "Before <think> thinking </think> middle <think> more </think> after"
    result = strip_thinking_blocks(content)

    assert result.content == "Before  middle  after"
    assert result.reasoning_stripped is True

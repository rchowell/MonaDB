"""Model adapter: recognizes pydantic and dataclasses by shape, imports neither."""

import dataclasses


def to_doc(value):
    """Normalize a value for storage.

    Dicts pass through untouched; dataclass instances and pydantic models are
    converted to dicts. Anything else passes through for the Rust layer to
    reject with a TypeError naming the offending path.
    """
    if isinstance(value, dict):
        return value
    if hasattr(type(value), "model_dump"):  # pydantic, by duck-typing
        return value.model_dump()
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return dataclasses.asdict(value)
    return value


def from_doc(model, doc):
    """Rehydrate a stored dict as `model`, or return it unchanged if unbound."""
    if model is None:
        return doc
    if hasattr(model, "model_validate"):  # pydantic, by duck-typing
        return model.model_validate(doc)
    if dataclasses.is_dataclass(model):
        return model(**doc)
    raise TypeError(f"unsupported model type: {model!r}")

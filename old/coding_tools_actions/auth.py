from __future__ import annotations

import secrets

from fastapi import Header, HTTPException, Request, status


async def require_request_auth(
    request: Request,
    authorization: str | None = Header(default=None),
) -> None:
    auth_type = request.app.state.settings.auth_type
    if auth_type == "none":
        return
    if auth_type == "oauth":
        raise HTTPException(
            status_code=status.HTTP_501_NOT_IMPLEMENTED,
            detail="OAuth is not available for the Actions gateway yet",
        )

    expected = request.app.state.settings.api_key
    if not expected:
        raise RuntimeError("ACTIONS_API_KEY is not configured")

    if not authorization:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Missing Authorization header",
        )

    scheme, separator, value = authorization.partition(" ")
    if (
        not separator
        or scheme.lower() != "bearer"
        or not secrets.compare_digest(value.strip(), expected)
    ):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid API key",
        )

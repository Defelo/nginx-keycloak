# nginx-keycloak
[Keycloak](https://www.keycloak.org/) Integration for [Nginx](https://nginx.org/) via [`auth_request`](https://nginx.org/en/docs/http/ngx_http_auth_request_module.html)

Restricts access to Nginx sites by requiring users to authenticate with their Keycloak account. Only users with a specific role (configurable per service) are granted access.

## Setup Instructions

### Keycloak
1. Create a new `OpenID Connect` client in Keycloak
2. Enable `Client authentication`
3. Create Redirect URIs for your services (e.g. `https://<DOMAIN>/_auth/callback`, see `AUTH_CALLBACK` environment variable)
4. Copy secret from `Credentials` tab
5. Go to `Client scopes` &rarr; `CLIENT_ID-dedicated` and add a predefined `client roles` mapper:
    - Set `Client ID` to the id of your client
    - Set `Token Claim Name` to `roles`
    - Disable `Add to access token`
    - Enable `Add to userinfo`
    - Save
6. Create client roles for your services

### nginx-keycloak.env
1. Update your `ISSUER` url
2. Set `CLIENT_ID` to your client id and `CLIENT_SECRET` to your client secret
3. (*optional*) Change `AUTH_CALLBACK` to a different path it the default conflicts with one of your services
4. (*optional*) Adjust `SESSION_ALLOWED_TTL` and `SESSION_FORBIDDEN_TTL`

### Nginx
1. Make sure your nginx includes the [`ngx_http_auth_request_module`](https://nginx.org/en/docs/http/ngx_http_auth_request_module.html):
    ```sh
    nginx -V 2>&1 | grep http_auth_request_module
    ```
2. Create an internal `location` block in your server:
    ```nginx
    location /_auth/keycloak {
        internal;
        proxy_pass http://CONTAINER_HOST:CONTAINER_PORT/auth?role=SERVICE_ROLE_NAME;
        proxy_pass_request_body off;
        proxy_set_header Content-Length "";
        proxy_set_header X-Request-Uri $scheme://$host$request_uri;
    }
    ```
3. Add the following to any `location` block you want to control access to:
    ```nginx
    auth_request /_auth/keycloak;
    auth_request_set $auth_redirect $upstream_http_x_auth_redirect;
    auth_request_set $auth_cookie $upstream_http_x_auth_cookie;
    error_page 401 =307 $auth_redirect;
    add_header Set-Cookie $auth_cookie;
    ```

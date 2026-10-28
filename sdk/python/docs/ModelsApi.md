# chess_nnue_lab.ModelsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**activate_model**](ModelsApi.md#activate_model) | **POST** /api/v1/models/{id}:activate | Makes the engine play (and analyse) with this model, from the next search on.
[**deactivate_model**](ModelsApi.md#deactivate_model) | **POST** /api/v1/models:deactivate | Goes back to the server&#39;s configured network.
[**delete_model**](ModelsApi.md#delete_model) | **DELETE** /api/v1/models/{id} | Deletes a model and its file.
[**download_model**](ModelsApi.md#download_model) | **GET** /api/v1/models/{id}/file | The network file (&#x60;CBNN&#x60; version 2), loadable by the engine (&#x60;setoption name EvalFile&#x60;).
[**get_model**](ModelsApi.md#get_model) | **GET** /api/v1/models/{id} | A model, with how it was made.
[**list_models**](ModelsApi.md#list_models) | **GET** /api/v1/models | Models, newest first.


# **activate_model**
> ModelView activate_model(id)

Makes the engine play (and analyse) with this model, from the next search on.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.model_view import ModelView
from chess_nnue_lab.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = chess_nnue_lab.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure API key authorization: api_key
configuration.api_key['api_key'] = os.environ["API_KEY"]

# Uncomment below to setup prefix (e.g. Bearer) for API key, if needed
# configuration.api_key_prefix['api_key'] = 'Bearer'

# Enter a context with an instance of the API client
with chess_nnue_lab.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = chess_nnue_lab.ModelsApi(api_client)
    id = 'id_example' # str | Model id

    try:
        # Makes the engine play (and analyse) with this model, from the next search on.
        api_response = api_instance.activate_model(id)
        print("The response of ModelsApi->activate_model:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ModelsApi->activate_model: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Model id | 

### Return type

[**ModelView**](ModelView.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Active |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **deactivate_model**
> deactivate_model()

Goes back to the server's configured network.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = chess_nnue_lab.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure API key authorization: api_key
configuration.api_key['api_key'] = os.environ["API_KEY"]

# Uncomment below to setup prefix (e.g. Bearer) for API key, if needed
# configuration.api_key_prefix['api_key'] = 'Bearer'

# Enter a context with an instance of the API client
with chess_nnue_lab.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = chess_nnue_lab.ModelsApi(api_client)

    try:
        # Goes back to the server's configured network.
        api_instance.deactivate_model()
    except Exception as e:
        print("Exception when calling ModelsApi->deactivate_model: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

void (empty response body)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**204** | No model is active any more |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **delete_model**
> delete_model(id)

Deletes a model and its file.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = chess_nnue_lab.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure API key authorization: api_key
configuration.api_key['api_key'] = os.environ["API_KEY"]

# Uncomment below to setup prefix (e.g. Bearer) for API key, if needed
# configuration.api_key_prefix['api_key'] = 'Bearer'

# Enter a context with an instance of the API client
with chess_nnue_lab.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = chess_nnue_lab.ModelsApi(api_client)
    id = 'id_example' # str | Model id

    try:
        # Deletes a model and its file.
        api_instance.delete_model(id)
    except Exception as e:
        print("Exception when calling ModelsApi->delete_model: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Model id | 

### Return type

void (empty response body)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**204** | Deleted |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**409** | It is the active model |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **download_model**
> bytes download_model(id)

The network file (`CBNN` version 2), loadable by the engine (`setoption name EvalFile`).

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = chess_nnue_lab.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure API key authorization: api_key
configuration.api_key['api_key'] = os.environ["API_KEY"]

# Uncomment below to setup prefix (e.g. Bearer) for API key, if needed
# configuration.api_key_prefix['api_key'] = 'Bearer'

# Enter a context with an instance of the API client
with chess_nnue_lab.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = chess_nnue_lab.ModelsApi(api_client)
    id = 'id_example' # str | Model id

    try:
        # The network file (`CBNN` version 2), loadable by the engine (`setoption name EvalFile`).
        api_response = api_instance.download_model(id)
        print("The response of ModelsApi->download_model:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ModelsApi->download_model: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Model id | 

### Return type

**bytes**

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/octet-stream, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The network |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_model**
> ModelView get_model(id)

A model, with how it was made.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.model_view import ModelView
from chess_nnue_lab.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = chess_nnue_lab.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure API key authorization: api_key
configuration.api_key['api_key'] = os.environ["API_KEY"]

# Uncomment below to setup prefix (e.g. Bearer) for API key, if needed
# configuration.api_key_prefix['api_key'] = 'Bearer'

# Enter a context with an instance of the API client
with chess_nnue_lab.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = chess_nnue_lab.ModelsApi(api_client)
    id = 'id_example' # str | Model id

    try:
        # A model, with how it was made.
        api_response = api_instance.get_model(id)
        print("The response of ModelsApi->get_model:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ModelsApi->get_model: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Model id | 

### Return type

[**ModelView**](ModelView.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The model |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_models**
> ModelPage list_models(limit=limit, cursor=cursor)

Models, newest first.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.model_page import ModelPage
from chess_nnue_lab.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = chess_nnue_lab.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure API key authorization: api_key
configuration.api_key['api_key'] = os.environ["API_KEY"]

# Uncomment below to setup prefix (e.g. Bearer) for API key, if needed
# configuration.api_key_prefix['api_key'] = 'Bearer'

# Enter a context with an instance of the API client
with chess_nnue_lab.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = chess_nnue_lab.ModelsApi(api_client)
    limit = 56 # int | Items per page, 1 to 200 (default 50). (optional)
    cursor = 'cursor_example' # str | `nextCursor` of the previous page. (optional)

    try:
        # Models, newest first.
        api_response = api_instance.list_models(limit=limit, cursor=cursor)
        print("The response of ModelsApi->list_models:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ModelsApi->list_models: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **int**| Items per page, 1 to 200 (default 50). | [optional] 
 **cursor** | **str**| &#x60;nextCursor&#x60; of the previous page. | [optional] 

### Return type

[**ModelPage**](ModelPage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | A page of models |  -  |
**400** | Invalid cursor or limit |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


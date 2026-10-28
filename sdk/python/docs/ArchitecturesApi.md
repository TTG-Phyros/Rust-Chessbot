# chess_nnue_lab.ArchitecturesApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_architecture**](ArchitecturesApi.md#create_architecture) | **POST** /api/v1/architectures | Saves an architecture for training runs to use (&#x60;architectureId&#x60;).
[**delete_architecture**](ArchitecturesApi.md#delete_architecture) | **DELETE** /api/v1/architectures/{id} | Deletes a saved architecture (runs and models keep their copy).
[**get_architecture**](ArchitecturesApi.md#get_architecture) | **GET** /api/v1/architectures/{id} | A saved architecture.
[**get_architecture_schema**](ArchitecturesApi.md#get_architecture_schema) | **GET** /api/v1/architectures/schema | The JSON Schema of architecture documents (for editors: &#x60;$schema&#x60;).
[**list_architectures**](ArchitecturesApi.md#list_architectures) | **GET** /api/v1/architectures | Saved architectures.
[**list_presets**](ArchitecturesApi.md#list_presets) | **GET** /api/v1/architectures/presets | The ready-made architectures.
[**validate_architecture**](ArchitecturesApi.md#validate_architecture) | **POST** /api/v1/architectures:validate | Checks an architecture document without saving it.


# **create_architecture**
> StoredArchitecture create_architecture(architecture, idempotency_key=idempotency_key)

Saves an architecture for training runs to use (`architectureId`).

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.architecture import Architecture
from chess_nnue_lab.models.stored_architecture import StoredArchitecture
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
    api_instance = chess_nnue_lab.ArchitecturesApi(api_client)
    architecture = chess_nnue_lab.Architecture() # Architecture | Architecture document
    idempotency_key = 'idempotency_key_example' # str | Makes retries safe (optional)

    try:
        # Saves an architecture for training runs to use (`architectureId`).
        api_response = api_instance.create_architecture(architecture, idempotency_key=idempotency_key)
        print("The response of ArchitecturesApi->create_architecture:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ArchitecturesApi->create_architecture: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **architecture** | [**Architecture**](Architecture.md)| Architecture document | 
 **idempotency_key** | **str**| Makes retries safe | [optional] 

### Return type

[**StoredArchitecture**](StoredArchitecture.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: application/json, application/yaml
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Saved |  -  |
**401** | Missing or invalid API key |  -  |
**422** | Invalid architecture, with every error |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **delete_architecture**
> delete_architecture(id)

Deletes a saved architecture (runs and models keep their copy).

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
    api_instance = chess_nnue_lab.ArchitecturesApi(api_client)
    id = 'id_example' # str | Architecture id

    try:
        # Deletes a saved architecture (runs and models keep their copy).
        api_instance.delete_architecture(id)
    except Exception as e:
        print("Exception when calling ArchitecturesApi->delete_architecture: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Architecture id | 

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
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_architecture**
> StoredArchitecture get_architecture(id)

A saved architecture.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.stored_architecture import StoredArchitecture
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
    api_instance = chess_nnue_lab.ArchitecturesApi(api_client)
    id = 'id_example' # str | Architecture id

    try:
        # A saved architecture.
        api_response = api_instance.get_architecture(id)
        print("The response of ArchitecturesApi->get_architecture:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ArchitecturesApi->get_architecture: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Architecture id | 

### Return type

[**StoredArchitecture**](StoredArchitecture.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The architecture |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_architecture_schema**
> object get_architecture_schema()

The JSON Schema of architecture documents (for editors: `$schema`).

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
    api_instance = chess_nnue_lab.ArchitecturesApi(api_client)

    try:
        # The JSON Schema of architecture documents (for editors: `$schema`).
        api_response = api_instance.get_architecture_schema()
        print("The response of ArchitecturesApi->get_architecture_schema:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ArchitecturesApi->get_architecture_schema: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

**object**

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/schema+json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | JSON Schema 2020-12 |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_architectures**
> ArchitecturePage list_architectures(limit=limit, cursor=cursor)

Saved architectures.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.architecture_page import ArchitecturePage
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
    api_instance = chess_nnue_lab.ArchitecturesApi(api_client)
    limit = 56 # int | Items per page, 1 to 200 (default 50). (optional)
    cursor = 'cursor_example' # str | `nextCursor` of the previous page. (optional)

    try:
        # Saved architectures.
        api_response = api_instance.list_architectures(limit=limit, cursor=cursor)
        print("The response of ArchitecturesApi->list_architectures:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ArchitecturesApi->list_architectures: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **int**| Items per page, 1 to 200 (default 50). | [optional] 
 **cursor** | **str**| &#x60;nextCursor&#x60; of the previous page. | [optional] 

### Return type

[**ArchitecturePage**](ArchitecturePage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | A page of saved architectures |  -  |
**400** | Invalid cursor or limit |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_presets**
> List[Preset] list_presets()

The ready-made architectures.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.preset import Preset
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
    api_instance = chess_nnue_lab.ArchitecturesApi(api_client)

    try:
        # The ready-made architectures.
        api_response = api_instance.list_presets()
        print("The response of ArchitecturesApi->list_presets:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ArchitecturesApi->list_presets: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**List[Preset]**](Preset.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Every preset |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **validate_architecture**
> ValidationReport validate_architecture(architecture)

Checks an architecture document without saving it.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.architecture import Architecture
from chess_nnue_lab.models.validation_report import ValidationReport
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
    api_instance = chess_nnue_lab.ArchitecturesApi(api_client)
    architecture = chess_nnue_lab.Architecture() # Architecture | Architecture document

    try:
        # Checks an architecture document without saving it.
        api_response = api_instance.validate_architecture(architecture)
        print("The response of ArchitecturesApi->validate_architecture:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ArchitecturesApi->validate_architecture: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **architecture** | [**Architecture**](Architecture.md)| Architecture document | 

### Return type

[**ValidationReport**](ValidationReport.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: application/json, application/yaml
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Validation result, valid or not |  -  |
**400** | Not JSON or YAML |  -  |
**401** | Missing or invalid API key |  -  |
**415** | Unsupported media type |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


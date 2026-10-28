# chess_nnue_lab.DatasetsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**delete_dataset**](DatasetsApi.md#delete_dataset) | **DELETE** /api/v1/datasets/{id} | Deletes a dataset and its files.
[**get_dataset**](DatasetsApi.md#get_dataset) | **GET** /api/v1/datasets/{id} | A dataset.
[**get_dataset_stats**](DatasetsApi.md#get_dataset_stats) | **GET** /api/v1/datasets/{id}/stats | Statistics of a ready dataset.
[**list_datasets**](DatasetsApi.md#list_datasets) | **GET** /api/v1/datasets | Datasets.
[**start_selfplay**](DatasetsApi.md#start_selfplay) | **POST** /api/v1/datasets:selfplay | Starts a self-play run that generates a new dataset.
[**upload_dataset**](DatasetsApi.md#upload_dataset) | **POST** /api/v1/datasets | Uploads a dataset file (&#x60;CBDS&#x60;, see the documentation). It is checked record by record while it streams to disk; nothing is kept if it is invalid.


# **delete_dataset**
> delete_dataset(id)

Deletes a dataset and its files.

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
    api_instance = chess_nnue_lab.DatasetsApi(api_client)
    id = 'id_example' # str | Dataset id

    try:
        # Deletes a dataset and its files.
        api_instance.delete_dataset(id)
    except Exception as e:
        print("Exception when calling DatasetsApi->delete_dataset: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Dataset id | 

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
**409** | Being generated, or used by a queued or running training |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_dataset**
> Dataset get_dataset(id)

A dataset.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.dataset import Dataset
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
    api_instance = chess_nnue_lab.DatasetsApi(api_client)
    id = 'id_example' # str | Dataset id

    try:
        # A dataset.
        api_response = api_instance.get_dataset(id)
        print("The response of DatasetsApi->get_dataset:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling DatasetsApi->get_dataset: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Dataset id | 

### Return type

[**Dataset**](Dataset.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The dataset |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_dataset_stats**
> DatasetStats get_dataset_stats(id)

Statistics of a ready dataset.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.dataset_stats import DatasetStats
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
    api_instance = chess_nnue_lab.DatasetsApi(api_client)
    id = 'id_example' # str | Dataset id

    try:
        # Statistics of a ready dataset.
        api_response = api_instance.get_dataset_stats(id)
        print("The response of DatasetsApi->get_dataset_stats:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling DatasetsApi->get_dataset_stats: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Dataset id | 

### Return type

[**DatasetStats**](DatasetStats.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Statistics |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**409** | Not ready yet, or failed |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_datasets**
> DatasetPage list_datasets(limit=limit, cursor=cursor)

Datasets.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.dataset_page import DatasetPage
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
    api_instance = chess_nnue_lab.DatasetsApi(api_client)
    limit = 56 # int | Items per page, 1 to 200 (default 50). (optional)
    cursor = 'cursor_example' # str | `nextCursor` of the previous page. (optional)

    try:
        # Datasets.
        api_response = api_instance.list_datasets(limit=limit, cursor=cursor)
        print("The response of DatasetsApi->list_datasets:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling DatasetsApi->list_datasets: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **int**| Items per page, 1 to 200 (default 50). | [optional] 
 **cursor** | **str**| &#x60;nextCursor&#x60; of the previous page. | [optional] 

### Return type

[**DatasetPage**](DatasetPage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | A page of datasets |  -  |
**400** | Invalid cursor or limit |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **start_selfplay**
> SelfplayStarted start_selfplay(selfplay_request, idempotency_key=idempotency_key)

Starts a self-play run that generates a new dataset.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.selfplay_request import SelfplayRequest
from chess_nnue_lab.models.selfplay_started import SelfplayStarted
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
    api_instance = chess_nnue_lab.DatasetsApi(api_client)
    selfplay_request = chess_nnue_lab.SelfplayRequest() # SelfplayRequest | 
    idempotency_key = 'idempotency_key_example' # str | Makes retries safe (optional)

    try:
        # Starts a self-play run that generates a new dataset.
        api_response = api_instance.start_selfplay(selfplay_request, idempotency_key=idempotency_key)
        print("The response of DatasetsApi->start_selfplay:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling DatasetsApi->start_selfplay: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **selfplay_request** | [**SelfplayRequest**](SelfplayRequest.md)|  | 
 **idempotency_key** | **str**| Makes retries safe | [optional] 

### Return type

[**SelfplayStarted**](SelfplayStarted.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**202** | Queued |  -  |
**401** | Missing or invalid API key |  -  |
**422** | Invalid request, with every error |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **upload_dataset**
> Dataset upload_dataset(name, body, idempotency_key=idempotency_key)

Uploads a dataset file (`CBDS`, see the documentation). It is checked record by record while it streams to disk; nothing is kept if it is invalid.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.dataset import Dataset
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
    api_instance = chess_nnue_lab.DatasetsApi(api_client)
    name = 'name_example' # str | Name of the dataset (1 to 100 characters).
    body = None # bytes | The CBDS file
    idempotency_key = 'idempotency_key_example' # str | Makes retries safe (optional)

    try:
        # Uploads a dataset file (`CBDS`, see the documentation). It is checked record by record while it streams to disk; nothing is kept if it is invalid.
        api_response = api_instance.upload_dataset(name, body, idempotency_key=idempotency_key)
        print("The response of DatasetsApi->upload_dataset:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling DatasetsApi->upload_dataset: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **name** | **str**| Name of the dataset (1 to 100 characters). | 
 **body** | **bytes**| The CBDS file | 
 **idempotency_key** | **str**| Makes retries safe | [optional] 

### Return type

[**Dataset**](Dataset.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: application/octet-stream
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Stored, with its hash and statistics |  -  |
**401** | Missing or invalid API key |  -  |
**413** | Larger than the server&#39;s limit |  -  |
**415** | Not application/octet-stream |  -  |
**422** | Invalid name or file |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


# chess_nnue_lab.SelfplayRunsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_selfplay_run**](SelfplayRunsApi.md#get_selfplay_run) | **GET** /api/v1/selfplay-runs/{id} | A self-play run (start one with &#x60;POST /api/v1/datasets:selfplay&#x60;).
[**list_selfplay_runs**](SelfplayRunsApi.md#list_selfplay_runs) | **GET** /api/v1/selfplay-runs | Self-play runs.
[**stop_selfplay_run**](SelfplayRunsApi.md#stop_selfplay_run) | **POST** /api/v1/selfplay-runs/{id}:stop | Stops a self-play run: its dataset keeps the games already played.
[**stream_selfplay_events**](SelfplayRunsApi.md#stream_selfplay_events) | **GET** /api/v1/selfplay-runs/{id}/events | Live events of a self-play run (see the training runs&#39; events).


# **get_selfplay_run**
> Job get_selfplay_run(id)

A self-play run (start one with `POST /api/v1/datasets:selfplay`).

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.job import Job
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
    api_instance = chess_nnue_lab.SelfplayRunsApi(api_client)
    id = 'id_example' # str | Run id

    try:
        # A self-play run (start one with `POST /api/v1/datasets:selfplay`).
        api_response = api_instance.get_selfplay_run(id)
        print("The response of SelfplayRunsApi->get_selfplay_run:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SelfplayRunsApi->get_selfplay_run: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Run id | 

### Return type

[**Job**](Job.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The run |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_selfplay_runs**
> JobPage list_selfplay_runs(limit=limit, cursor=cursor)

Self-play runs.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.job_page import JobPage
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
    api_instance = chess_nnue_lab.SelfplayRunsApi(api_client)
    limit = 56 # int | Items per page, 1 to 200 (default 50). (optional)
    cursor = 'cursor_example' # str | `nextCursor` of the previous page. (optional)

    try:
        # Self-play runs.
        api_response = api_instance.list_selfplay_runs(limit=limit, cursor=cursor)
        print("The response of SelfplayRunsApi->list_selfplay_runs:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SelfplayRunsApi->list_selfplay_runs: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **int**| Items per page, 1 to 200 (default 50). | [optional] 
 **cursor** | **str**| &#x60;nextCursor&#x60; of the previous page. | [optional] 

### Return type

[**JobPage**](JobPage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | A page of runs |  -  |
**400** | Invalid cursor or limit |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **stop_selfplay_run**
> Job stop_selfplay_run(id)

Stops a self-play run: its dataset keeps the games already played.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.job import Job
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
    api_instance = chess_nnue_lab.SelfplayRunsApi(api_client)
    id = 'id_example' # str | Run id

    try:
        # Stops a self-play run: its dataset keeps the games already played.
        api_response = api_instance.stop_selfplay_run(id)
        print("The response of SelfplayRunsApi->stop_selfplay_run:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SelfplayRunsApi->stop_selfplay_run: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Run id | 

### Return type

[**Job**](Job.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | &#x60;stopping&#x60; (or &#x60;canceled&#x60;) |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**409** | Already over |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **stream_selfplay_events**
> str stream_selfplay_events(id, after=after, last_event_id=last_event_id)

Live events of a self-play run (see the training runs' events).

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
    api_instance = chess_nnue_lab.SelfplayRunsApi(api_client)
    id = 'id_example' # str | Run id
    after = 56 # int | Sequence number of the last event received: the stream resumes after it (as the `Last-Event-ID` header does). (optional)
    last_event_id = 'last_event_id_example' # str | Resume after this event (optional)

    try:
        # Live events of a self-play run (see the training runs' events).
        api_response = api_instance.stream_selfplay_events(id, after=after, last_event_id=last_event_id)
        print("The response of SelfplayRunsApi->stream_selfplay_events:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SelfplayRunsApi->stream_selfplay_events: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Run id | 
 **after** | **int**| Sequence number of the last event received: the stream resumes after it (as the &#x60;Last-Event-ID&#x60; header does). | [optional] 
 **last_event_id** | **str**| Resume after this event | [optional] 

### Return type

**str**

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: text/event-stream, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Event stream; each &#x60;data&#x60; is a JobEvent (JSON) |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


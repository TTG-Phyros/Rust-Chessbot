# chess_nnue_lab.TrainingRunsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_training_run**](TrainingRunsApi.md#create_training_run) | **POST** /api/v1/training-runs | Queues a training run. Everything needed to reproduce it is recorded: the normalized architecture, the content hash of each dataset, the hyperparameters and the seed (drawn when not given).
[**get_training_metrics**](TrainingRunsApi.md#get_training_metrics) | **GET** /api/v1/training-runs/{id}/metrics | The training curve: loss, learning rate and speed per superbatch.
[**get_training_run**](TrainingRunsApi.md#get_training_run) | **GET** /api/v1/training-runs/{id} | A training run, with its latest progress.
[**list_training_runs**](TrainingRunsApi.md#list_training_runs) | **GET** /api/v1/training-runs | Training runs.
[**resume_training_run**](TrainingRunsApi.md#resume_training_run) | **POST** /api/v1/training-runs/{id}:resume | Queues a stopped, interrupted or failed training run again: it resumes from its last checkpoint.
[**stop_training_run**](TrainingRunsApi.md#stop_training_run) | **POST** /api/v1/training-runs/{id}:stop | Stops a training run: a queued run is canceled, a running one saves a checkpoint at the end of its superbatch and stops.
[**stream_training_events**](TrainingRunsApi.md#stream_training_events) | **GET** /api/v1/training-runs/{id}/events | Live events of a training run (Server-Sent Events): &#x60;status&#x60;, &#x60;progress&#x60;, &#x60;checkpoint&#x60;, &#x60;model&#x60;, &#x60;log&#x60;. Each event&#39;s &#x60;id&#x60; is its sequence number; reconnecting with &#x60;Last-Event-ID&#x60; (or &#x60;after&#x60;) resumes after it. The stream replays the run&#39;s history first, and ends once the run is over.


# **create_training_run**
> Job create_training_run(training_request, idempotency_key=idempotency_key)

Queues a training run. Everything needed to reproduce it is recorded: the normalized architecture, the content hash of each dataset, the hyperparameters and the seed (drawn when not given).

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.job import Job
from chess_nnue_lab.models.training_request import TrainingRequest
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
    api_instance = chess_nnue_lab.TrainingRunsApi(api_client)
    training_request = chess_nnue_lab.TrainingRequest() # TrainingRequest | 
    idempotency_key = 'idempotency_key_example' # str | Makes retries safe (optional)

    try:
        # Queues a training run. Everything needed to reproduce it is recorded: the normalized architecture, the content hash of each dataset, the hyperparameters and the seed (drawn when not given).
        api_response = api_instance.create_training_run(training_request, idempotency_key=idempotency_key)
        print("The response of TrainingRunsApi->create_training_run:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TrainingRunsApi->create_training_run: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **training_request** | [**TrainingRequest**](TrainingRequest.md)|  | 
 **idempotency_key** | **str**| Makes retries safe | [optional] 

### Return type

[**Job**](Job.md)

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

# **get_training_metrics**
> Metrics get_training_metrics(id)

The training curve: loss, learning rate and speed per superbatch.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.metrics import Metrics
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
    api_instance = chess_nnue_lab.TrainingRunsApi(api_client)
    id = 'id_example' # str | Run id

    try:
        # The training curve: loss, learning rate and speed per superbatch.
        api_response = api_instance.get_training_metrics(id)
        print("The response of TrainingRunsApi->get_training_metrics:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TrainingRunsApi->get_training_metrics: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Run id | 

### Return type

[**Metrics**](Metrics.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The curve |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_training_run**
> Job get_training_run(id)

A training run, with its latest progress.

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
    api_instance = chess_nnue_lab.TrainingRunsApi(api_client)
    id = 'id_example' # str | Run id

    try:
        # A training run, with its latest progress.
        api_response = api_instance.get_training_run(id)
        print("The response of TrainingRunsApi->get_training_run:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TrainingRunsApi->get_training_run: %s\n" % e)
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

# **list_training_runs**
> JobPage list_training_runs(limit=limit, cursor=cursor)

Training runs.

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
    api_instance = chess_nnue_lab.TrainingRunsApi(api_client)
    limit = 56 # int | Items per page, 1 to 200 (default 50). (optional)
    cursor = 'cursor_example' # str | `nextCursor` of the previous page. (optional)

    try:
        # Training runs.
        api_response = api_instance.list_training_runs(limit=limit, cursor=cursor)
        print("The response of TrainingRunsApi->list_training_runs:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TrainingRunsApi->list_training_runs: %s\n" % e)
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

# **resume_training_run**
> Job resume_training_run(id)

Queues a stopped, interrupted or failed training run again: it resumes from its last checkpoint.

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
    api_instance = chess_nnue_lab.TrainingRunsApi(api_client)
    id = 'id_example' # str | Run id

    try:
        # Queues a stopped, interrupted or failed training run again: it resumes from its last checkpoint.
        api_response = api_instance.resume_training_run(id)
        print("The response of TrainingRunsApi->resume_training_run:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TrainingRunsApi->resume_training_run: %s\n" % e)
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
**200** | Queued |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**409** | Not stopped, interrupted or failed |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **stop_training_run**
> Job stop_training_run(id)

Stops a training run: a queued run is canceled, a running one saves a checkpoint at the end of its superbatch and stops.

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
    api_instance = chess_nnue_lab.TrainingRunsApi(api_client)
    id = 'id_example' # str | Run id

    try:
        # Stops a training run: a queued run is canceled, a running one saves a checkpoint at the end of its superbatch and stops.
        api_response = api_instance.stop_training_run(id)
        print("The response of TrainingRunsApi->stop_training_run:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TrainingRunsApi->stop_training_run: %s\n" % e)
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

# **stream_training_events**
> str stream_training_events(id, after=after, last_event_id=last_event_id)

Live events of a training run (Server-Sent Events): `status`, `progress`, `checkpoint`, `model`, `log`. Each event's `id` is its sequence number; reconnecting with `Last-Event-ID` (or `after`) resumes after it. The stream replays the run's history first, and ends once the run is over.

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
    api_instance = chess_nnue_lab.TrainingRunsApi(api_client)
    id = 'id_example' # str | Run id
    after = 56 # int | Sequence number of the last event received: the stream resumes after it (as the `Last-Event-ID` header does). (optional)
    last_event_id = 'last_event_id_example' # str | Resume after this event (optional)

    try:
        # Live events of a training run (Server-Sent Events): `status`, `progress`, `checkpoint`, `model`, `log`. Each event's `id` is its sequence number; reconnecting with `Last-Event-ID` (or `after`) resumes after it. The stream replays the run's history first, and ends once the run is over.
        api_response = api_instance.stream_training_events(id, after=after, last_event_id=last_event_id)
        print("The response of TrainingRunsApi->stream_training_events:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TrainingRunsApi->stream_training_events: %s\n" % e)
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


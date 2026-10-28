# chess_nnue_lab.EvaluationsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**compare_models**](EvaluationsApi.md#compare_models) | **GET** /api/v1/evaluations:compare | Compares models (and the classical evaluation) over every evaluation played between two of them, whatever its settings: network size, speed, results of each pair, and Elo ratings fitted over all the games (Bradley–Terry maximum likelihood, relative to the first player).
[**create_evaluation**](EvaluationsApi.md#create_evaluation) | **POST** /api/v1/evaluations | Queues an evaluation match: the candidate model plays pairs of games against the baseline (a model, or the classical evaluation) from the same random opening with colors swapped, at a fixed number of nodes (reproducible) or time per move. With &#x60;sprt&#x60;, the match stops as soon as the test concludes. The worker first measures the speed (nodes per second) of both engines.
[**get_evaluation**](EvaluationsApi.md#get_evaluation) | **GET** /api/v1/evaluations/{id} | An evaluation, with its latest result: W/D/L, Elo difference ± its 95 % error, likelihood of superiority, SPRT status, and the speed of both engines. Its &#x60;spec&#x60; describes both contestants (name, architecture, size), even once their models are deleted.
[**list_evaluations**](EvaluationsApi.md#list_evaluations) | **GET** /api/v1/evaluations | Evaluations, newest first.
[**stop_evaluation**](EvaluationsApi.md#stop_evaluation) | **POST** /api/v1/evaluations/{id}:stop | Stops an evaluation: a queued one is canceled, a running one stops after its current games and keeps their result.
[**stream_evaluation_events**](EvaluationsApi.md#stream_evaluation_events) | **GET** /api/v1/evaluations/{id}/events | Live events of an evaluation (Server-Sent Events): &#x60;status&#x60;, and a &#x60;progress&#x60; after each batch of games. See the training runs&#39; events.


# **compare_models**
> Comparison compare_models(players)

Compares models (and the classical evaluation) over every evaluation played between two of them, whatever its settings: network size, speed, results of each pair, and Elo ratings fitted over all the games (Bradley–Terry maximum likelihood, relative to the first player).

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.comparison import Comparison
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
    api_instance = chess_nnue_lab.EvaluationsApi(api_client)
    players = 'players_example' # str | Comma-separated model ids, 1 to 16; `classic` stands for the classical evaluation. Ratings are relative to the first.

    try:
        # Compares models (and the classical evaluation) over every evaluation played between two of them, whatever its settings: network size, speed, results of each pair, and Elo ratings fitted over all the games (Bradley–Terry maximum likelihood, relative to the first player).
        api_response = api_instance.compare_models(players)
        print("The response of EvaluationsApi->compare_models:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EvaluationsApi->compare_models: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **players** | **str**| Comma-separated model ids, 1 to 16; &#x60;classic&#x60; stands for the classical evaluation. Ratings are relative to the first. | 

### Return type

[**Comparison**](Comparison.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json, application/problem+json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The comparison |  -  |
**400** | Invalid list of players |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown model |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **create_evaluation**
> Job create_evaluation(evaluation_request, idempotency_key=idempotency_key)

Queues an evaluation match: the candidate model plays pairs of games against the baseline (a model, or the classical evaluation) from the same random opening with colors swapped, at a fixed number of nodes (reproducible) or time per move. With `sprt`, the match stops as soon as the test concludes. The worker first measures the speed (nodes per second) of both engines.

### Example

* Api Key Authentication (api_key):

```python
import chess_nnue_lab
from chess_nnue_lab.models.evaluation_request import EvaluationRequest
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
    api_instance = chess_nnue_lab.EvaluationsApi(api_client)
    evaluation_request = chess_nnue_lab.EvaluationRequest() # EvaluationRequest | 
    idempotency_key = 'idempotency_key_example' # str | Makes retries safe (optional)

    try:
        # Queues an evaluation match: the candidate model plays pairs of games against the baseline (a model, or the classical evaluation) from the same random opening with colors swapped, at a fixed number of nodes (reproducible) or time per move. With `sprt`, the match stops as soon as the test concludes. The worker first measures the speed (nodes per second) of both engines.
        api_response = api_instance.create_evaluation(evaluation_request, idempotency_key=idempotency_key)
        print("The response of EvaluationsApi->create_evaluation:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EvaluationsApi->create_evaluation: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **evaluation_request** | [**EvaluationRequest**](EvaluationRequest.md)|  | 
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

# **get_evaluation**
> Job get_evaluation(id)

An evaluation, with its latest result: W/D/L, Elo difference ± its 95 % error, likelihood of superiority, SPRT status, and the speed of both engines. Its `spec` describes both contestants (name, architecture, size), even once their models are deleted.

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
    api_instance = chess_nnue_lab.EvaluationsApi(api_client)
    id = 'id_example' # str | Evaluation id

    try:
        # An evaluation, with its latest result: W/D/L, Elo difference ± its 95 % error, likelihood of superiority, SPRT status, and the speed of both engines. Its `spec` describes both contestants (name, architecture, size), even once their models are deleted.
        api_response = api_instance.get_evaluation(id)
        print("The response of EvaluationsApi->get_evaluation:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EvaluationsApi->get_evaluation: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Evaluation id | 

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
**200** | The evaluation |  -  |
**401** | Missing or invalid API key |  -  |
**404** | Unknown id |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_evaluations**
> JobPage list_evaluations(limit=limit, cursor=cursor)

Evaluations, newest first.

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
    api_instance = chess_nnue_lab.EvaluationsApi(api_client)
    limit = 56 # int | Items per page, 1 to 200 (default 50). (optional)
    cursor = 'cursor_example' # str | `nextCursor` of the previous page. (optional)

    try:
        # Evaluations, newest first.
        api_response = api_instance.list_evaluations(limit=limit, cursor=cursor)
        print("The response of EvaluationsApi->list_evaluations:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EvaluationsApi->list_evaluations: %s\n" % e)
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
**200** | A page of evaluations |  -  |
**400** | Invalid cursor or limit |  -  |
**401** | Missing or invalid API key |  -  |
**429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **stop_evaluation**
> Job stop_evaluation(id)

Stops an evaluation: a queued one is canceled, a running one stops after its current games and keeps their result.

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
    api_instance = chess_nnue_lab.EvaluationsApi(api_client)
    id = 'id_example' # str | Evaluation id

    try:
        # Stops an evaluation: a queued one is canceled, a running one stops after its current games and keeps their result.
        api_response = api_instance.stop_evaluation(id)
        print("The response of EvaluationsApi->stop_evaluation:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EvaluationsApi->stop_evaluation: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Evaluation id | 

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

# **stream_evaluation_events**
> str stream_evaluation_events(id, after=after, last_event_id=last_event_id)

Live events of an evaluation (Server-Sent Events): `status`, and a `progress` after each batch of games. See the training runs' events.

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
    api_instance = chess_nnue_lab.EvaluationsApi(api_client)
    id = 'id_example' # str | Evaluation id
    after = 56 # int | Sequence number of the last event received: the stream resumes after it (as the `Last-Event-ID` header does). (optional)
    last_event_id = 'last_event_id_example' # str | Resume after this event (optional)

    try:
        # Live events of an evaluation (Server-Sent Events): `status`, and a `progress` after each batch of games. See the training runs' events.
        api_response = api_instance.stream_evaluation_events(id, after=after, last_event_id=last_event_id)
        print("The response of EvaluationsApi->stream_evaluation_events:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EvaluationsApi->stream_evaluation_events: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **str**| Evaluation id | 
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


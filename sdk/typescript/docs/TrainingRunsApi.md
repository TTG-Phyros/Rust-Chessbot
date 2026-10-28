# TrainingRunsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createTrainingRun**](TrainingRunsApi.md#createtrainingrun) | **POST** /api/v1/training-runs | Queues a training run. Everything needed to reproduce it is recorded: the normalized architecture, the content hash of each dataset, the hyperparameters and the seed (drawn when not given). |
| [**getTrainingMetrics**](TrainingRunsApi.md#gettrainingmetrics) | **GET** /api/v1/training-runs/{id}/metrics | The training curve: loss, learning rate and speed per superbatch. |
| [**getTrainingRun**](TrainingRunsApi.md#gettrainingrun) | **GET** /api/v1/training-runs/{id} | A training run, with its latest progress. |
| [**listTrainingRuns**](TrainingRunsApi.md#listtrainingruns) | **GET** /api/v1/training-runs | Training runs. |
| [**resumeTrainingRun**](TrainingRunsApi.md#resumetrainingrun) | **POST** /api/v1/training-runs/{id}:resume | Queues a stopped, interrupted or failed training run again: it resumes from its last checkpoint. |
| [**stopTrainingRun**](TrainingRunsApi.md#stoptrainingrun) | **POST** /api/v1/training-runs/{id}:stop | Stops a training run: a queued run is canceled, a running one saves a checkpoint at the end of its superbatch and stops. |
| [**streamTrainingEvents**](TrainingRunsApi.md#streamtrainingevents) | **GET** /api/v1/training-runs/{id}/events | Live events of a training run (Server-Sent Events): &#x60;status&#x60;, &#x60;progress&#x60;, &#x60;checkpoint&#x60;, &#x60;model&#x60;, &#x60;log&#x60;. Each event\&#39;s &#x60;id&#x60; is its sequence number; reconnecting with &#x60;Last-Event-ID&#x60; (or &#x60;after&#x60;) resumes after it. The stream replays the run\&#39;s history first, and ends once the run is over. |



## createTrainingRun

> Job createTrainingRun(trainingRequest, idempotencyKey)

Queues a training run. Everything needed to reproduce it is recorded: the normalized architecture, the content hash of each dataset, the hyperparameters and the seed (drawn when not given).

### Example

```ts
import {
  Configuration,
  TrainingRunsApi,
} from 'chess-nnue-lab';
import type { CreateTrainingRunRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new TrainingRunsApi(config);

  const body = {
    // TrainingRequest
    trainingRequest: ...,
    // string | Makes retries safe (optional)
    idempotencyKey: idempotencyKey_example,
  } satisfies CreateTrainingRunRequest;

  try {
    const data = await api.createTrainingRun(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **trainingRequest** | [TrainingRequest](TrainingRequest.md) |  | |
| **idempotencyKey** | `string` | Makes retries safe | [Optional] [Defaults to `undefined`] |

### Return type

[**Job**](Job.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **202** | Queued |  -  |
| **401** | Missing or invalid API key |  -  |
| **422** | Invalid request, with every error |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getTrainingMetrics

> Metrics getTrainingMetrics(id)

The training curve: loss, learning rate and speed per superbatch.

### Example

```ts
import {
  Configuration,
  TrainingRunsApi,
} from 'chess-nnue-lab';
import type { GetTrainingMetricsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new TrainingRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
  } satisfies GetTrainingMetricsRequest;

  try {
    const data = await api.getTrainingMetrics(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **id** | `string` | Run id | [Defaults to `undefined`] |

### Return type

[**Metrics**](Metrics.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The curve |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getTrainingRun

> Job getTrainingRun(id)

A training run, with its latest progress.

### Example

```ts
import {
  Configuration,
  TrainingRunsApi,
} from 'chess-nnue-lab';
import type { GetTrainingRunRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new TrainingRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
  } satisfies GetTrainingRunRequest;

  try {
    const data = await api.getTrainingRun(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **id** | `string` | Run id | [Defaults to `undefined`] |

### Return type

[**Job**](Job.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The run |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listTrainingRuns

> JobPage listTrainingRuns(limit, cursor)

Training runs.

### Example

```ts
import {
  Configuration,
  TrainingRunsApi,
} from 'chess-nnue-lab';
import type { ListTrainingRunsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new TrainingRunsApi(config);

  const body = {
    // number | Items per page, 1 to 200 (default 50). (optional)
    limit: 56,
    // string | `nextCursor` of the previous page. (optional)
    cursor: cursor_example,
  } satisfies ListTrainingRunsRequest;

  try {
    const data = await api.listTrainingRuns(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **limit** | `number` | Items per page, 1 to 200 (default 50). | [Optional] [Defaults to `undefined`] |
| **cursor** | `string` | &#x60;nextCursor&#x60; of the previous page. | [Optional] [Defaults to `undefined`] |

### Return type

[**JobPage**](JobPage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | A page of runs |  -  |
| **400** | Invalid cursor or limit |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## resumeTrainingRun

> Job resumeTrainingRun(id)

Queues a stopped, interrupted or failed training run again: it resumes from its last checkpoint.

### Example

```ts
import {
  Configuration,
  TrainingRunsApi,
} from 'chess-nnue-lab';
import type { ResumeTrainingRunRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new TrainingRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
  } satisfies ResumeTrainingRunRequest;

  try {
    const data = await api.resumeTrainingRun(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **id** | `string` | Run id | [Defaults to `undefined`] |

### Return type

[**Job**](Job.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Queued |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **409** | Not stopped, interrupted or failed |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## stopTrainingRun

> Job stopTrainingRun(id)

Stops a training run: a queued run is canceled, a running one saves a checkpoint at the end of its superbatch and stops.

### Example

```ts
import {
  Configuration,
  TrainingRunsApi,
} from 'chess-nnue-lab';
import type { StopTrainingRunRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new TrainingRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
  } satisfies StopTrainingRunRequest;

  try {
    const data = await api.stopTrainingRun(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **id** | `string` | Run id | [Defaults to `undefined`] |

### Return type

[**Job**](Job.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | &#x60;stopping&#x60; (or &#x60;canceled&#x60;) |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **409** | Already over |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## streamTrainingEvents

> string streamTrainingEvents(id, after, lastEventID)

Live events of a training run (Server-Sent Events): &#x60;status&#x60;, &#x60;progress&#x60;, &#x60;checkpoint&#x60;, &#x60;model&#x60;, &#x60;log&#x60;. Each event\&#39;s &#x60;id&#x60; is its sequence number; reconnecting with &#x60;Last-Event-ID&#x60; (or &#x60;after&#x60;) resumes after it. The stream replays the run\&#39;s history first, and ends once the run is over.

### Example

```ts
import {
  Configuration,
  TrainingRunsApi,
} from 'chess-nnue-lab';
import type { StreamTrainingEventsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new TrainingRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
    // number | Sequence number of the last event received: the stream resumes after it (as the `Last-Event-ID` header does). (optional)
    after: 789,
    // string | Resume after this event (optional)
    lastEventID: lastEventID_example,
  } satisfies StreamTrainingEventsRequest;

  try {
    const data = await api.streamTrainingEvents(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **id** | `string` | Run id | [Defaults to `undefined`] |
| **after** | `number` | Sequence number of the last event received: the stream resumes after it (as the &#x60;Last-Event-ID&#x60; header does). | [Optional] [Defaults to `undefined`] |
| **lastEventID** | `string` | Resume after this event | [Optional] [Defaults to `undefined`] |

### Return type

**string**

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `text/event-stream`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Event stream; each &#x60;data&#x60; is a JobEvent (JSON) |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


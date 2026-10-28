# EvaluationsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**compareModels**](EvaluationsApi.md#comparemodels) | **GET** /api/v1/evaluations:compare | Compares models (and the classical evaluation) over every evaluation played between two of them, whatever its settings: network size, speed, results of each pair, and Elo ratings fitted over all the games (Bradley–Terry maximum likelihood, relative to the first player). |
| [**createEvaluation**](EvaluationsApi.md#createevaluation) | **POST** /api/v1/evaluations | Queues an evaluation match: the candidate model plays pairs of games against the baseline (a model, or the classical evaluation) from the same random opening with colors swapped, at a fixed number of nodes (reproducible) or time per move. With &#x60;sprt&#x60;, the match stops as soon as the test concludes. The worker first measures the speed (nodes per second) of both engines. |
| [**getEvaluation**](EvaluationsApi.md#getevaluation) | **GET** /api/v1/evaluations/{id} | An evaluation, with its latest result: W/D/L, Elo difference ± its 95 % error, likelihood of superiority, SPRT status, and the speed of both engines. Its &#x60;spec&#x60; describes both contestants (name, architecture, size), even once their models are deleted. |
| [**listEvaluations**](EvaluationsApi.md#listevaluations) | **GET** /api/v1/evaluations | Evaluations, newest first. |
| [**stopEvaluation**](EvaluationsApi.md#stopevaluation) | **POST** /api/v1/evaluations/{id}:stop | Stops an evaluation: a queued one is canceled, a running one stops after its current games and keeps their result. |
| [**streamEvaluationEvents**](EvaluationsApi.md#streamevaluationevents) | **GET** /api/v1/evaluations/{id}/events | Live events of an evaluation (Server-Sent Events): &#x60;status&#x60;, and a &#x60;progress&#x60; after each batch of games. See the training runs\&#39; events. |



## compareModels

> Comparison compareModels(players)

Compares models (and the classical evaluation) over every evaluation played between two of them, whatever its settings: network size, speed, results of each pair, and Elo ratings fitted over all the games (Bradley–Terry maximum likelihood, relative to the first player).

### Example

```ts
import {
  Configuration,
  EvaluationsApi,
} from 'chess-nnue-lab';
import type { CompareModelsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new EvaluationsApi(config);

  const body = {
    // string | Comma-separated model ids, 1 to 16; `classic` stands for the classical evaluation. Ratings are relative to the first.
    players: players_example,
  } satisfies CompareModelsRequest;

  try {
    const data = await api.compareModels(body);
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
| **players** | `string` | Comma-separated model ids, 1 to 16; &#x60;classic&#x60; stands for the classical evaluation. Ratings are relative to the first. | [Defaults to `undefined`] |

### Return type

[**Comparison**](Comparison.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The comparison |  -  |
| **400** | Invalid list of players |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown model |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## createEvaluation

> Job createEvaluation(evaluationRequest, idempotencyKey)

Queues an evaluation match: the candidate model plays pairs of games against the baseline (a model, or the classical evaluation) from the same random opening with colors swapped, at a fixed number of nodes (reproducible) or time per move. With &#x60;sprt&#x60;, the match stops as soon as the test concludes. The worker first measures the speed (nodes per second) of both engines.

### Example

```ts
import {
  Configuration,
  EvaluationsApi,
} from 'chess-nnue-lab';
import type { CreateEvaluationRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new EvaluationsApi(config);

  const body = {
    // EvaluationRequest
    evaluationRequest: ...,
    // string | Makes retries safe (optional)
    idempotencyKey: idempotencyKey_example,
  } satisfies CreateEvaluationRequest;

  try {
    const data = await api.createEvaluation(body);
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
| **evaluationRequest** | [EvaluationRequest](EvaluationRequest.md) |  | |
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


## getEvaluation

> Job getEvaluation(id)

An evaluation, with its latest result: W/D/L, Elo difference ± its 95 % error, likelihood of superiority, SPRT status, and the speed of both engines. Its &#x60;spec&#x60; describes both contestants (name, architecture, size), even once their models are deleted.

### Example

```ts
import {
  Configuration,
  EvaluationsApi,
} from 'chess-nnue-lab';
import type { GetEvaluationRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new EvaluationsApi(config);

  const body = {
    // string | Evaluation id
    id: id_example,
  } satisfies GetEvaluationRequest;

  try {
    const data = await api.getEvaluation(body);
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
| **id** | `string` | Evaluation id | [Defaults to `undefined`] |

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
| **200** | The evaluation |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listEvaluations

> JobPage listEvaluations(limit, cursor)

Evaluations, newest first.

### Example

```ts
import {
  Configuration,
  EvaluationsApi,
} from 'chess-nnue-lab';
import type { ListEvaluationsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new EvaluationsApi(config);

  const body = {
    // number | Items per page, 1 to 200 (default 50). (optional)
    limit: 56,
    // string | `nextCursor` of the previous page. (optional)
    cursor: cursor_example,
  } satisfies ListEvaluationsRequest;

  try {
    const data = await api.listEvaluations(body);
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
| **200** | A page of evaluations |  -  |
| **400** | Invalid cursor or limit |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## stopEvaluation

> Job stopEvaluation(id)

Stops an evaluation: a queued one is canceled, a running one stops after its current games and keeps their result.

### Example

```ts
import {
  Configuration,
  EvaluationsApi,
} from 'chess-nnue-lab';
import type { StopEvaluationRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new EvaluationsApi(config);

  const body = {
    // string | Evaluation id
    id: id_example,
  } satisfies StopEvaluationRequest;

  try {
    const data = await api.stopEvaluation(body);
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
| **id** | `string` | Evaluation id | [Defaults to `undefined`] |

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


## streamEvaluationEvents

> string streamEvaluationEvents(id, after, lastEventID)

Live events of an evaluation (Server-Sent Events): &#x60;status&#x60;, and a &#x60;progress&#x60; after each batch of games. See the training runs\&#39; events.

### Example

```ts
import {
  Configuration,
  EvaluationsApi,
} from 'chess-nnue-lab';
import type { StreamEvaluationEventsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new EvaluationsApi(config);

  const body = {
    // string | Evaluation id
    id: id_example,
    // number | Sequence number of the last event received: the stream resumes after it (as the `Last-Event-ID` header does). (optional)
    after: 789,
    // string | Resume after this event (optional)
    lastEventID: lastEventID_example,
  } satisfies StreamEvaluationEventsRequest;

  try {
    const data = await api.streamEvaluationEvents(body);
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
| **id** | `string` | Evaluation id | [Defaults to `undefined`] |
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


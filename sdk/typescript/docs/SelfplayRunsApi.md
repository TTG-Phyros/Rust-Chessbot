# SelfplayRunsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**getSelfplayRun**](SelfplayRunsApi.md#getselfplayrun) | **GET** /api/v1/selfplay-runs/{id} | A self-play run (start one with &#x60;POST /api/v1/datasets:selfplay&#x60;). |
| [**listSelfplayRuns**](SelfplayRunsApi.md#listselfplayruns) | **GET** /api/v1/selfplay-runs | Self-play runs. |
| [**stopSelfplayRun**](SelfplayRunsApi.md#stopselfplayrun) | **POST** /api/v1/selfplay-runs/{id}:stop | Stops a self-play run: its dataset keeps the games already played. |
| [**streamSelfplayEvents**](SelfplayRunsApi.md#streamselfplayevents) | **GET** /api/v1/selfplay-runs/{id}/events | Live events of a self-play run (see the training runs\&#39; events). |



## getSelfplayRun

> Job getSelfplayRun(id)

A self-play run (start one with &#x60;POST /api/v1/datasets:selfplay&#x60;).

### Example

```ts
import {
  Configuration,
  SelfplayRunsApi,
} from 'chess-nnue-lab';
import type { GetSelfplayRunRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new SelfplayRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
  } satisfies GetSelfplayRunRequest;

  try {
    const data = await api.getSelfplayRun(body);
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


## listSelfplayRuns

> JobPage listSelfplayRuns(limit, cursor)

Self-play runs.

### Example

```ts
import {
  Configuration,
  SelfplayRunsApi,
} from 'chess-nnue-lab';
import type { ListSelfplayRunsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new SelfplayRunsApi(config);

  const body = {
    // number | Items per page, 1 to 200 (default 50). (optional)
    limit: 56,
    // string | `nextCursor` of the previous page. (optional)
    cursor: cursor_example,
  } satisfies ListSelfplayRunsRequest;

  try {
    const data = await api.listSelfplayRuns(body);
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


## stopSelfplayRun

> Job stopSelfplayRun(id)

Stops a self-play run: its dataset keeps the games already played.

### Example

```ts
import {
  Configuration,
  SelfplayRunsApi,
} from 'chess-nnue-lab';
import type { StopSelfplayRunRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new SelfplayRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
  } satisfies StopSelfplayRunRequest;

  try {
    const data = await api.stopSelfplayRun(body);
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


## streamSelfplayEvents

> string streamSelfplayEvents(id, after, lastEventID)

Live events of a self-play run (see the training runs\&#39; events).

### Example

```ts
import {
  Configuration,
  SelfplayRunsApi,
} from 'chess-nnue-lab';
import type { StreamSelfplayEventsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new SelfplayRunsApi(config);

  const body = {
    // string | Run id
    id: id_example,
    // number | Sequence number of the last event received: the stream resumes after it (as the `Last-Event-ID` header does). (optional)
    after: 789,
    // string | Resume after this event (optional)
    lastEventID: lastEventID_example,
  } satisfies StreamSelfplayEventsRequest;

  try {
    const data = await api.streamSelfplayEvents(body);
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


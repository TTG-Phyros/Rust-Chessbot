# ModelsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**activateModel**](ModelsApi.md#activatemodel) | **POST** /api/v1/models/{id}:activate | Makes the engine play (and analyse) with this model, from the next search on. |
| [**deactivateModel**](ModelsApi.md#deactivatemodel) | **POST** /api/v1/models:deactivate | Goes back to the server\&#39;s configured network. |
| [**deleteModel**](ModelsApi.md#deletemodel) | **DELETE** /api/v1/models/{id} | Deletes a model and its file. |
| [**downloadModel**](ModelsApi.md#downloadmodel) | **GET** /api/v1/models/{id}/file | The network file (&#x60;CBNN&#x60; version 2), loadable by the engine (&#x60;setoption name EvalFile&#x60;). |
| [**getModel**](ModelsApi.md#getmodel) | **GET** /api/v1/models/{id} | A model, with how it was made. |
| [**listModels**](ModelsApi.md#listmodels) | **GET** /api/v1/models | Models, newest first. |



## activateModel

> ModelView activateModel(id)

Makes the engine play (and analyse) with this model, from the next search on.

### Example

```ts
import {
  Configuration,
  ModelsApi,
} from 'chess-nnue-lab';
import type { ActivateModelRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ModelsApi(config);

  const body = {
    // string | Model id
    id: id_example,
  } satisfies ActivateModelRequest;

  try {
    const data = await api.activateModel(body);
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
| **id** | `string` | Model id | [Defaults to `undefined`] |

### Return type

[**ModelView**](ModelView.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Active |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## deactivateModel

> deactivateModel()

Goes back to the server\&#39;s configured network.

### Example

```ts
import {
  Configuration,
  ModelsApi,
} from 'chess-nnue-lab';
import type { DeactivateModelRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ModelsApi(config);

  try {
    const data = await api.deactivateModel();
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters

This endpoint does not need any parameter.

### Return type

`void` (Empty response body)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **204** | No model is active any more |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## deleteModel

> deleteModel(id)

Deletes a model and its file.

### Example

```ts
import {
  Configuration,
  ModelsApi,
} from 'chess-nnue-lab';
import type { DeleteModelRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ModelsApi(config);

  const body = {
    // string | Model id
    id: id_example,
  } satisfies DeleteModelRequest;

  try {
    const data = await api.deleteModel(body);
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
| **id** | `string` | Model id | [Defaults to `undefined`] |

### Return type

`void` (Empty response body)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **204** | Deleted |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **409** | It is the active model |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## downloadModel

> Blob downloadModel(id)

The network file (&#x60;CBNN&#x60; version 2), loadable by the engine (&#x60;setoption name EvalFile&#x60;).

### Example

```ts
import {
  Configuration,
  ModelsApi,
} from 'chess-nnue-lab';
import type { DownloadModelRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ModelsApi(config);

  const body = {
    // string | Model id
    id: id_example,
  } satisfies DownloadModelRequest;

  try {
    const data = await api.downloadModel(body);
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
| **id** | `string` | Model id | [Defaults to `undefined`] |

### Return type

**Blob**

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/octet-stream`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The network |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getModel

> ModelView getModel(id)

A model, with how it was made.

### Example

```ts
import {
  Configuration,
  ModelsApi,
} from 'chess-nnue-lab';
import type { GetModelRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ModelsApi(config);

  const body = {
    // string | Model id
    id: id_example,
  } satisfies GetModelRequest;

  try {
    const data = await api.getModel(body);
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
| **id** | `string` | Model id | [Defaults to `undefined`] |

### Return type

[**ModelView**](ModelView.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The model |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listModels

> ModelPage listModels(limit, cursor)

Models, newest first.

### Example

```ts
import {
  Configuration,
  ModelsApi,
} from 'chess-nnue-lab';
import type { ListModelsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ModelsApi(config);

  const body = {
    // number | Items per page, 1 to 200 (default 50). (optional)
    limit: 56,
    // string | `nextCursor` of the previous page. (optional)
    cursor: cursor_example,
  } satisfies ListModelsRequest;

  try {
    const data = await api.listModels(body);
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

[**ModelPage**](ModelPage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | A page of models |  -  |
| **400** | Invalid cursor or limit |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


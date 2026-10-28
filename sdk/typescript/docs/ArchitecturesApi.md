# ArchitecturesApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createArchitecture**](ArchitecturesApi.md#createarchitecture) | **POST** /api/v1/architectures | Saves an architecture for training runs to use (&#x60;architectureId&#x60;). |
| [**deleteArchitecture**](ArchitecturesApi.md#deletearchitecture) | **DELETE** /api/v1/architectures/{id} | Deletes a saved architecture (runs and models keep their copy). |
| [**getArchitecture**](ArchitecturesApi.md#getarchitecture) | **GET** /api/v1/architectures/{id} | A saved architecture. |
| [**getArchitectureSchema**](ArchitecturesApi.md#getarchitectureschema) | **GET** /api/v1/architectures/schema | The JSON Schema of architecture documents (for editors: &#x60;$schema&#x60;). |
| [**listArchitectures**](ArchitecturesApi.md#listarchitectures) | **GET** /api/v1/architectures | Saved architectures. |
| [**listPresets**](ArchitecturesApi.md#listpresets) | **GET** /api/v1/architectures/presets | The ready-made architectures. |
| [**validateArchitecture**](ArchitecturesApi.md#validatearchitecture) | **POST** /api/v1/architectures:validate | Checks an architecture document without saving it. |



## createArchitecture

> StoredArchitecture createArchitecture(architecture, idempotencyKey)

Saves an architecture for training runs to use (&#x60;architectureId&#x60;).

### Example

```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { CreateArchitectureRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  const body = {
    // Architecture | Architecture document
    architecture: ...,
    // string | Makes retries safe (optional)
    idempotencyKey: idempotencyKey_example,
  } satisfies CreateArchitectureRequest;

  try {
    const data = await api.createArchitecture(body);
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
| **architecture** | [Architecture](Architecture.md) | Architecture document | |
| **idempotencyKey** | `string` | Makes retries safe | [Optional] [Defaults to `undefined`] |

### Return type

[**StoredArchitecture**](StoredArchitecture.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: `application/json`, `application/yaml`
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Saved |  -  |
| **401** | Missing or invalid API key |  -  |
| **422** | Invalid architecture, with every error |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## deleteArchitecture

> deleteArchitecture(id)

Deletes a saved architecture (runs and models keep their copy).

### Example

```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { DeleteArchitectureRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  const body = {
    // string | Architecture id
    id: id_example,
  } satisfies DeleteArchitectureRequest;

  try {
    const data = await api.deleteArchitecture(body);
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
| **id** | `string` | Architecture id | [Defaults to `undefined`] |

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
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getArchitecture

> StoredArchitecture getArchitecture(id)

A saved architecture.

### Example

```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { GetArchitectureRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  const body = {
    // string | Architecture id
    id: id_example,
  } satisfies GetArchitectureRequest;

  try {
    const data = await api.getArchitecture(body);
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
| **id** | `string` | Architecture id | [Defaults to `undefined`] |

### Return type

[**StoredArchitecture**](StoredArchitecture.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The architecture |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getArchitectureSchema

> object getArchitectureSchema()

The JSON Schema of architecture documents (for editors: &#x60;$schema&#x60;).

### Example

```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { GetArchitectureSchemaRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  try {
    const data = await api.getArchitectureSchema();
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

**object**

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/schema+json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | JSON Schema 2020-12 |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listArchitectures

> ArchitecturePage listArchitectures(limit, cursor)

Saved architectures.

### Example

```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { ListArchitecturesRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  const body = {
    // number | Items per page, 1 to 200 (default 50). (optional)
    limit: 56,
    // string | `nextCursor` of the previous page. (optional)
    cursor: cursor_example,
  } satisfies ListArchitecturesRequest;

  try {
    const data = await api.listArchitectures(body);
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

[**ArchitecturePage**](ArchitecturePage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | A page of saved architectures |  -  |
| **400** | Invalid cursor or limit |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listPresets

> Array&lt;Preset&gt; listPresets()

The ready-made architectures.

### Example

```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { ListPresetsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  try {
    const data = await api.listPresets();
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

[**Array&lt;Preset&gt;**](Preset.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Every preset |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## validateArchitecture

> ValidationReport validateArchitecture(architecture)

Checks an architecture document without saving it.

### Example

```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { ValidateArchitectureRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  const body = {
    // Architecture | Architecture document
    architecture: ...,
  } satisfies ValidateArchitectureRequest;

  try {
    const data = await api.validateArchitecture(body);
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
| **architecture** | [Architecture](Architecture.md) | Architecture document | |

### Return type

[**ValidationReport**](ValidationReport.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: `application/json`, `application/yaml`
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Validation result, valid or not |  -  |
| **400** | Not JSON or YAML |  -  |
| **401** | Missing or invalid API key |  -  |
| **415** | Unsupported media type |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


# DatasetsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**deleteDataset**](DatasetsApi.md#deletedataset) | **DELETE** /api/v1/datasets/{id} | Deletes a dataset and its files. |
| [**getDataset**](DatasetsApi.md#getdataset) | **GET** /api/v1/datasets/{id} | A dataset. |
| [**getDatasetStats**](DatasetsApi.md#getdatasetstats) | **GET** /api/v1/datasets/{id}/stats | Statistics of a ready dataset. |
| [**listDatasets**](DatasetsApi.md#listdatasets) | **GET** /api/v1/datasets | Datasets. |
| [**startSelfplay**](DatasetsApi.md#startselfplay) | **POST** /api/v1/datasets:selfplay | Starts a self-play run that generates a new dataset. |
| [**uploadDataset**](DatasetsApi.md#uploaddataset) | **POST** /api/v1/datasets | Uploads a dataset file (&#x60;CBDS&#x60;, see the documentation). It is checked record by record while it streams to disk; nothing is kept if it is invalid. |



## deleteDataset

> deleteDataset(id)

Deletes a dataset and its files.

### Example

```ts
import {
  Configuration,
  DatasetsApi,
} from 'chess-nnue-lab';
import type { DeleteDatasetRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new DatasetsApi(config);

  const body = {
    // string | Dataset id
    id: id_example,
  } satisfies DeleteDatasetRequest;

  try {
    const data = await api.deleteDataset(body);
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
| **id** | `string` | Dataset id | [Defaults to `undefined`] |

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
| **409** | Being generated, or used by a queued or running training |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getDataset

> Dataset getDataset(id)

A dataset.

### Example

```ts
import {
  Configuration,
  DatasetsApi,
} from 'chess-nnue-lab';
import type { GetDatasetRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new DatasetsApi(config);

  const body = {
    // string | Dataset id
    id: id_example,
  } satisfies GetDatasetRequest;

  try {
    const data = await api.getDataset(body);
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
| **id** | `string` | Dataset id | [Defaults to `undefined`] |

### Return type

[**Dataset**](Dataset.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The dataset |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getDatasetStats

> DatasetStats getDatasetStats(id)

Statistics of a ready dataset.

### Example

```ts
import {
  Configuration,
  DatasetsApi,
} from 'chess-nnue-lab';
import type { GetDatasetStatsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new DatasetsApi(config);

  const body = {
    // string | Dataset id
    id: id_example,
  } satisfies GetDatasetStatsRequest;

  try {
    const data = await api.getDatasetStats(body);
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
| **id** | `string` | Dataset id | [Defaults to `undefined`] |

### Return type

[**DatasetStats**](DatasetStats.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Statistics |  -  |
| **401** | Missing or invalid API key |  -  |
| **404** | Unknown id |  -  |
| **409** | Not ready yet, or failed |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listDatasets

> DatasetPage listDatasets(limit, cursor)

Datasets.

### Example

```ts
import {
  Configuration,
  DatasetsApi,
} from 'chess-nnue-lab';
import type { ListDatasetsRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new DatasetsApi(config);

  const body = {
    // number | Items per page, 1 to 200 (default 50). (optional)
    limit: 56,
    // string | `nextCursor` of the previous page. (optional)
    cursor: cursor_example,
  } satisfies ListDatasetsRequest;

  try {
    const data = await api.listDatasets(body);
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

[**DatasetPage**](DatasetPage.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | A page of datasets |  -  |
| **400** | Invalid cursor or limit |  -  |
| **401** | Missing or invalid API key |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## startSelfplay

> SelfplayStarted startSelfplay(selfplayRequest, idempotencyKey)

Starts a self-play run that generates a new dataset.

### Example

```ts
import {
  Configuration,
  DatasetsApi,
} from 'chess-nnue-lab';
import type { StartSelfplayRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new DatasetsApi(config);

  const body = {
    // SelfplayRequest
    selfplayRequest: ...,
    // string | Makes retries safe (optional)
    idempotencyKey: idempotencyKey_example,
  } satisfies StartSelfplayRequest;

  try {
    const data = await api.startSelfplay(body);
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
| **selfplayRequest** | [SelfplayRequest](SelfplayRequest.md) |  | |
| **idempotencyKey** | `string` | Makes retries safe | [Optional] [Defaults to `undefined`] |

### Return type

[**SelfplayStarted**](SelfplayStarted.md)

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


## uploadDataset

> Dataset uploadDataset(name, body, idempotencyKey)

Uploads a dataset file (&#x60;CBDS&#x60;, see the documentation). It is checked record by record while it streams to disk; nothing is kept if it is invalid.

### Example

```ts
import {
  Configuration,
  DatasetsApi,
} from 'chess-nnue-lab';
import type { UploadDatasetRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new DatasetsApi(config);

  const body = {
    // string | Name of the dataset (1 to 100 characters).
    name: name_example,
    // Blob | The CBDS file
    body: BINARY_DATA_HERE,
    // string | Makes retries safe (optional)
    idempotencyKey: idempotencyKey_example,
  } satisfies UploadDatasetRequest;

  try {
    const data = await api.uploadDataset(body);
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
| **name** | `string` | Name of the dataset (1 to 100 characters). | [Defaults to `undefined`] |
| **body** | `Blob` | The CBDS file | |
| **idempotencyKey** | `string` | Makes retries safe | [Optional] [Defaults to `undefined`] |

### Return type

[**Dataset**](Dataset.md)

### Authorization

[api_key](../README.md#api_key)

### HTTP request headers

- **Content-Type**: `application/octet-stream`
- **Accept**: `application/json`, `application/problem+json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Stored, with its hash and statistics |  -  |
| **401** | Missing or invalid API key |  -  |
| **413** | Larger than the server\&#39;s limit |  -  |
| **415** | Not application/octet-stream |  -  |
| **422** | Invalid name or file |  -  |
| **429** | Too many requests: wait &#x60;Retry-After&#x60; seconds |  * Retry-After -  <br>  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


# DatasetPage


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**items** | [**List[Dataset]**](Dataset.md) | Items of the page. | 
**next_cursor** | **str** | Cursor of the next page; absent on the last one. | [optional] 

## Example

```python
from chess_nnue_lab.models.dataset_page import DatasetPage

# TODO update the JSON string below
json = "{}"
# create an instance of DatasetPage from a JSON string
dataset_page_instance = DatasetPage.from_json(json)
# print the JSON string representation of the object
print(DatasetPage.to_json())

# convert the object into a dict
dataset_page_dict = dataset_page_instance.to_dict()
# create an instance of DatasetPage from a dict
dataset_page_from_dict = DatasetPage.from_dict(dataset_page_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



# ProgressOneOf1

See [`SelfplayProgress`].

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**games** | **int** | Games played. | 
**games_total** | **int** | Games to play. | 
**positions** | **int** | Positions recorded. | 
**kind** | **str** |  | 

## Example

```python
from chess_nnue_lab.models.progress_one_of1 import ProgressOneOf1

# TODO update the JSON string below
json = "{}"
# create an instance of ProgressOneOf1 from a JSON string
progress_one_of1_instance = ProgressOneOf1.from_json(json)
# print the JSON string representation of the object
print(ProgressOneOf1.to_json())

# convert the object into a dict
progress_one_of1_dict = progress_one_of1_instance.to_dict()
# create an instance of ProgressOneOf1 from a dict
progress_one_of1_from_dict = ProgressOneOf1.from_dict(progress_one_of1_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



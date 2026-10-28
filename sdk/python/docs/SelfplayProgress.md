# SelfplayProgress

Latest progress of a self-play job.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**games** | **int** | Games played. | 
**games_total** | **int** | Games to play. | 
**positions** | **int** | Positions recorded. | 

## Example

```python
from chess_nnue_lab.models.selfplay_progress import SelfplayProgress

# TODO update the JSON string below
json = "{}"
# create an instance of SelfplayProgress from a JSON string
selfplay_progress_instance = SelfplayProgress.from_json(json)
# print the JSON string representation of the object
print(SelfplayProgress.to_json())

# convert the object into a dict
selfplay_progress_dict = selfplay_progress_instance.to_dict()
# create an instance of SelfplayProgress from a dict
selfplay_progress_from_dict = SelfplayProgress.from_dict(selfplay_progress_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



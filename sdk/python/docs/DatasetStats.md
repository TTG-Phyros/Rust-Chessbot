# DatasetStats

What a dataset contains.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**mean_abs_score** | **float** | Mean absolute search score, in centipawns. | 
**outcomes** | [**Outcomes**](Outcomes.md) | Results of the games the positions come from. | 
**pieces** | **List[int]** | &#x60;pieces[n]&#x60;: positions with &#x60;n&#x60; pieces, kings included (0 to 32). | 
**plies** | [**Histogram**](Histogram.md) | Plies of the positions in their games, bins of 20 up to 400. | 
**records** | **int** | Positions. | 
**scores** | [**Histogram**](Histogram.md) | Search scores (White&#39;s point of view), bins of 100 centipawns from −1500 to +1500. | 
**white_to_move** | **int** | Positions with White to move. | 

## Example

```python
from chess_nnue_lab.models.dataset_stats import DatasetStats

# TODO update the JSON string below
json = "{}"
# create an instance of DatasetStats from a JSON string
dataset_stats_instance = DatasetStats.from_json(json)
# print the JSON string representation of the object
print(DatasetStats.to_json())

# convert the object into a dict
dataset_stats_dict = dataset_stats_instance.to_dict()
# create an instance of DatasetStats from a dict
dataset_stats_from_dict = DatasetStats.from_dict(dataset_stats_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)



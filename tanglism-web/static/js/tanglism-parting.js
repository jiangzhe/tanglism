// 定义分型相关函数
// 依赖jquery, jquery-ui, d3, tanglism-common, tanglism-kline
// 界面将不再显示分型数据
export const parting = {
    data,
    table,
    clear_table,
    ajax,
    outdate
};

// 分型数据
const _data = [];
// 数据是否过时
var _outdate = true;

function data(input) {
  if (input) {
    while(_data.length > 0) { _data.pop(); }
    for (var i = 0; i < input.length; i++) {
      _data.push(input[i]);
    }
    // 数据已被刷新
    _outdate = false;
    return;
  }
  return _data;
};

function table() {
  // 创建表格
  if (!d3.select("#p_table").empty()) {
    d3.select("#p_table").remove();
  }
  var table = d3.select("#p_container").append("table")
    .attr("id", "p_table")
    .style("border-collapse", "collapse")
    .style("border", "2px black solid");
  // 表头
  table.append("thead")
    .append("tr")
    .selectAll("th")
    .data(["峰值时刻", "峰值价格", "起始时刻", "结束时刻", "K线数目", "类型"])
    .enter()
    .append("th")
    .text(function(d) {return d;})
    .style("border", "1px black solid")
    .style("padding", "5px")
    .style("background-color", "lightgray")
    .style("font-weight", "bold");
  // 内容
  table.append("tbody")
    .selectAll("tr")
    .data(_data)
    .enter()
    .append("tr")
    .selectAll("td")
    .data(function(d) {
      return [d.extremum_ts, d.extremum_price, d.start_ts, d.end_ts, d.n, d.top ? "顶分型" : "底分型"];
    })
    .enter()
    .append("td")
    .style("border", "1px black solid")
    .style("padding", "5px")
    .style("font-size", "12px")
    .text(function(d) {return d;})
    .on("mouseover", function(){
      d3.select(this).style("background-color", "powderblue");
    })
    .on("mouseout", function(){
      d3.select(this).style("background-color", "white");
    });
};

function clear_table() {
    // 删除表格
    d3.select("#p_table").remove();
};

function ajax(params) {
    $.ajax({
      url: "api/v1/tanglism/partings/" + encodeURIComponent(params.code)
        + "/ticks/" + encodeURIComponent(params.tick) 
        + "?start_dt=" + encodeURIComponent(params.start_dt) 
        + "&end_dt=" + encodeURIComponent(params.end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        data(resp.data);
        table();
      },
      error: function(err) {
        console.log("ajax error on query partings", err);
        clear_table();
      }
    });
};

function outdate() {
    _outdate = true;
}